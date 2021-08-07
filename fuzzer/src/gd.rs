use crate::task::Fut;
use crate::task::Cons;
use crate::mut_input::MutInput;
use crate::grad::Grad;
use fastgen_common::config;
use crate::op_def::*;
use num_traits::FromPrimitive;
use std::collections::HashMap;


pub fn add_results(input: &MutInput, rgd_solutions: &mut Vec<HashMap<u32,u8>>,
    inputs: &Vec<(u32,u8)>, shape: &HashMap<u32,u32>) {
  let mut sol = HashMap::<u32,u8>::new();
  let mut ordered = Vec::<(u32,u64)>::new();
  let mut i = 0;
  for kv in inputs {
    ordered.push((kv.0, input.get(i)));
    i += 1;
  }
  ordered.sort_by_key(|k| k.0);
  i = 0;
  while i > ordered.len() {
    if shape[&ordered[i].0] != 0 {
      let start = ordered[i].0; 
      let mut res = ordered[i].1;
      let length = shape[&ordered[i].0];
      for k in 1..length {
        res += (ordered[i + k as usize].1) << (8*k);
      }
      for j in 0..length {
        sol.insert(start+j as u32, (res & 0xFF) as u8);
        res = res >> 8;
      }
      i += length as usize;
    } else {
      i = i + 1;
    }
  }
  rgd_solutions.push(sol); 
}


pub fn flip_op(comp: u32) -> u32 {
  let op = match FromPrimitive::from_u32(comp) {
    Some(RGD::Equal) => RGD::Distinct as u32,
      Some(RGD::Distinct) => RGD::Equal as u32,
      Some(RGD::Sge) => RGD::Slt as u32,
      Some(RGD::Sgt) => RGD::Sle as u32,
      Some(RGD::Sle) => RGD::Sgt as u32,
      Some(RGD::Slt) => RGD::Sge as u32,
      Some(RGD::Uge) => RGD::Ult as u32,
      Some(RGD::Ugt) => RGD::Ule as u32,
      Some(RGD::Ule) => RGD::Ugt as u32,
      Some(RGD::Ult) => RGD::Uge as u32,
      _ => panic!("Non-relational op!")
  };
  op
}


pub fn get_distance(comp: u32, a: u64, b: u64) -> u64 {
  let res = match FromPrimitive::from_u32(comp) {
    Some(RGD::Equal) => if a >= b { a - b } else { b - a },
      Some(RGD::Distinct) => if a == b { 1 } else { 0 },
      Some(RGD::Sge) => if (a as i64) >= (b as i64) { 0 } else { b - a },
      Some(RGD::Sgt) => if (a as i64) > (b as i64) { 0 } else { (b - a).saturating_add(1) },
      Some(RGD::Sle) => if (a as i64) <= (b as i64) { 0 } else { a - b },
      Some(RGD::Slt) => if (a as i64) < (b as i64) { 0 } else { (a-b).saturating_add(1) },
      Some(RGD::Uge) => if a >= b { 0 } else { b - a },
      Some(RGD::Ugt) => if a > b { 0 } else { (b - a).saturating_add(1) },
      Some(RGD::Ule) => if a <= b { 0 } else { a - b },
      Some(RGD::Ult) => if a < b { 0 } else { (a - b).saturating_add(1) },
      _ => panic!("Non-relational op!")
  };
  res
}


pub fn distance(min_input: &MutInput, constraints: &Vec<Cons>, 
    scratch_args: &mut Vec<u64>, distances: &mut Vec<u64>) -> u64 {
  let mut res: u64 = 0;
  let mut loop_count = 0;
  for cons in constraints {
    let mut arg_idx = 0;
    for arg in &cons.input_args {
      if arg.0 {
        scratch_args[2+arg_idx] = min_input.get(arg.1 as usize);
      } else {
        scratch_args[2+arg_idx] = arg.1
      }
      arg_idx += 1;
    }    
    cons.call_func(scratch_args);
    let mut comp = cons.comparison;
    if loop_count !=0  {
      comp = flip_op(comp);
    } 
    let dis = get_distance(comp, scratch_args[0], scratch_args[1]);
    distances[loop_count] = dis;
    if dis > 0 {
      res = res.saturating_add(dis);
    }
    loop_count += 1;
  }
  res
}

pub fn partial_derivative(orig_input: &mut MutInput, 
    scratch_args: &mut Vec<u64>,
    distances: &mut Vec<u64>,
    orig_distances: &Vec<u64>,
    f0: u64,
    index: usize,
    constraints: &Vec<Cons>,
    rgd_solutions: &mut Vec<HashMap<u32,u8>>,
    inputs: &Vec<(u32,u8)>, 
    shape: &HashMap<u32,u32>) -> (bool, bool, u64, bool) {
  let mut found = false;
  let orig_val = orig_input.get(index);

  orig_input.update(index, true, 1);
  let f_plus = distance(orig_input, constraints, scratch_args, distances);
  let plus_distances = distances.clone();
  if f_plus == 0 {
    add_results(orig_input, rgd_solutions, inputs, shape);
    found = true;
  }

  orig_input.set(index, orig_val);
  orig_input.update(index,false,1);
  let f_minus = distance(orig_input, constraints, scratch_args, distances);
  let minus_distances = distances.clone();
  if f_minus == 0 {
    add_results(orig_input, rgd_solutions, inputs, shape);
    found = true;
  }

  let mut res = match (f_minus < f0, f_plus < f0) {
    (false, false) => (true, false, 0, found),
      (false, true) => (
          true,
          f_minus != f0 && f_minus - f0 == f0 - f_plus,
          f0 - f_plus,
          found 
          ),
      (true, false) => (
          false,
          f_plus != f0 && f0 - f_minus == f_plus - f0,
          f0 - f_minus,
          found
          ),
      (true, true) => {
        if f_minus < f_plus {
          (false, false, f0 - f_minus, found)
        } else {
          (true, false, f0 - f_plus, found)
        }
      },
  };

  if res.2 == 0 {
    return res;
  }

  if res.0 {
    for i in 0..distances.len() {
      if plus_distances[i] !=0 && orig_distances[i] == 0 {
        orig_input.set_disable(index);
        res.2 = 0;
      }
    }
  } else {
    for i in 0..distances.len() {
      if minus_distances[i] !=0 && orig_distances[i] == 0 {
        orig_input.set_disable(index);
        res.2 = 0;
      }
    }
  }
  res
}

pub fn compute_delta_all(input: &mut MutInput, grad: &Grad, step: usize) {
  let step = step as f64;
  for (i, g) in grad.iter().enumerate() {
    let movement = g.pct * step;
    input.update(i, g.sign, movement as u64);
  }
}

pub fn guess_descend(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let rgd_solutions = &mut fut.rgd_solutions;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  let input_min = &mut ctx.min_input;
  let scratch_args = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let grad = &mut ctx.grad;
  let mut scratch_input = input_min.clone();
  let vsum = grad.val_sum();
  let mut f_last = ctx.f_last;
  if vsum > 0 {
    let guess_step = f_last / vsum;
    compute_delta_all(&mut scratch_input, grad, guess_step as usize);
    let f_new = distance(&scratch_input, &fut.constraints, scratch_args, distances);
    ctx.att += 1;
    if f_new < f_last {
      *input_min = scratch_input;
      ctx.orig_distances = ctx.distances.clone();
      f_last = f_new;
    }
  }

  ctx.f_last = f_last;
  ctx.next_state = 3;

  if f_last == 0 {
    ctx.solved = true;
    add_results(input_min, rgd_solutions, inputs, shape);
  }
}

pub fn onedimension_descend(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let rgd_solutions = &mut fut.rgd_solutions;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  let input_min = &mut ctx.min_input;
  let scratch_args = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let grad = &mut ctx.grad;
  let mut scratch_input = input_min.clone();
  let mut f_last = ctx.f_last;
  let mut dimension_idx = ctx.dimension_idx;
  while dimension_idx < grad.len() {
    if grad[dimension_idx].pct < 0.01 {
      dimension_idx += 1;
      continue;
    }
    loop {
      let movement = grad[dimension_idx].pct * ctx.step as f64;
      scratch_input.update(dimension_idx, grad[dimension_idx].sign, movement as u64);
      let f_new = distance(&scratch_input, &fut.constraints, scratch_args, distances);
      ctx.att += 1;
      if f_new >= f_last && f_new !=0 {
        ctx.step = 1;
        break;
      } else if f_new == 0 {
        f_last = f_new;
        ctx.solved = true;
        ctx.orig_distances = distances.clone();
        add_results(&scratch_input, rgd_solutions, inputs, shape);
        ctx.next_state = 4;
        ctx.dimension_idx = dimension_idx; 
        *input_min = scratch_input.clone();
        break;
      } else {
        *input_min = scratch_input.clone();
        f_last = f_new;
        ctx.orig_distances = distances.clone();
        ctx.step *= 2;
      }
    } 
    dimension_idx += 1;
  }
  ctx.f_last = f_last;
  if !ctx.solved {
    ctx.next_state = 1; //go to cal gradient
    ctx.dimension_idx = 0;
  }
}

pub fn alldimension_descend(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let rgd_solutions = &mut fut.rgd_solutions;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  let input_min = &mut ctx.min_input;
  let scratch_args = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let grad = &mut ctx.grad;
  let mut scratch_input = input_min.clone();
  let mut f_last = ctx.f_last;
  loop {
    compute_delta_all(&mut scratch_input, grad, ctx.step);
    let f_new = distance(&scratch_input, &fut.constraints, scratch_args, distances);
    ctx.att += 1;
    if f_new >= f_last && f_new !=0 {
      if grad.len() == 1 {
        ctx.next_state = 5;
      } else {
        ctx.next_state = 4;
      }
      ctx.step = 1;
      ctx.f_last = f_last;
      break;
    } else if f_new == 0 {
      ctx.solved = true;
      add_results(&scratch_input, rgd_solutions, inputs, shape);
      ctx.orig_distances = distances.clone();
      ctx.next_state = 3;
      ctx.f_last = f_last;
      *input_min = scratch_input.clone();
      break;
    } else {
      *input_min = scratch_input.clone();
      f_last = f_new;
      ctx.orig_distances = distances.clone();
      ctx.step *= 2;
    }
  } 
}


pub fn cal_gradient(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let rgd_solutions = &mut fut.rgd_solutions;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  let orig_input = &mut ctx.min_input;
  let scratch_args = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let orig_distances = & ctx.orig_distances;
  let f0 = ctx.f_last;
  let grad = &mut ctx.grad;
  for (i, g) in grad.iter_mut().enumerate() {
    let (sign, _islinear, val, solved) = partial_derivative(orig_input,scratch_args,
        distances, orig_distances, f0, i, 
        &fut.constraints, 
        rgd_solutions, inputs, shape);
    if solved {
      ctx.solved = true;
    }
    g.sign = sign;
    g.val = val;
  }
  ctx.att += ctx.grad.len();
  if ctx.grad.max_val() == 0 {
    ctx.next_state = 5; //ranomize 
  } else {
    ctx.next_state = 2;
    ctx.grad.normalize();
  }
}



pub fn repick_start_point(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let input_min = &mut ctx.min_input;
  let rgd_solutions = &mut fut.rgd_solutions;
  let scratch_args = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  input_min.randomize_all();
  input_min.reset_disable();
  ctx.f_last = distance(input_min, &fut.constraints, scratch_args, distances);
  ctx.orig_distances = ctx.distances.clone();
  ctx.next_state = 1; 
  ctx.grad.clear();
  ctx.att += 1;
  if ctx.f_last == 0 {
    ctx.solved = true;
    add_results(input_min, rgd_solutions, inputs, shape);
  }
}

pub fn load_input(fut: &mut Fut) {
  let mut ctx = fut.ctx.as_mut().unwrap();
  let input_min = &mut ctx.min_input;
  let rgd_solutions = &mut fut.rgd_solutions;
  let scratch_input = &mut ctx.scratch_args;
  let distances = &mut ctx.distances;
  let inputs = &fut.inputs;
  let shape = &fut.shape;
  input_min.assign(&fut.inputs);
  ctx.f_last = distance(input_min, &fut.constraints, scratch_input, distances);
  ctx.orig_distances = ctx.distances.clone();
  ctx.next_state = 1; 
  ctx.grad.clear();
  ctx.att += 1;
  if ctx.f_last == 0 {
    ctx.solved = true;
    add_results(input_min, rgd_solutions, inputs, shape);
  }
}

pub fn gd_search(fut: &mut Fut) -> bool {
  loop {
    let next_state = fut.ctx.as_ref().unwrap().next_state;
    match next_state {
      0 => {
        load_input(fut);
      },
        1 => {
          cal_gradient(fut);
        },
        2 => {
          guess_descend(fut);
        },
        3 => {
          alldimension_descend(fut);
        },
        4 => {
          onedimension_descend(fut);
        },
        5 => {
          repick_start_point(fut);
        },
        _ => (),
    } 
    let mut ctx = fut.ctx.as_mut().unwrap();
    if ctx.solved {
      ctx.solved = false;
      ctx.att = 0;
      return true;
    }
    if ctx.att > config::MAX_EXEC_TIMES {
      ctx.att = 0;
      return false;
    }
  }
}
