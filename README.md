# Fastgen

Fastgen (forked from Angora) is a continous concolic exection driver with a FIFO seed queue. In the front-end, it uses SymSan to collect the constraints. In the backend, it uses Z3 or JIGSAW as its solver.

# Experiments Results Reproduction


## Excution time without solving 

### nbench

1. Run Native

```
/src/nbench_native/nbench
```

2. Run SymSan

```
/src/nbench_symsan/nbench
```

3. Run SymCC

```
SYMCC_NO_SYMBOLIC_INPUT=1 /src/nbench_symcc/nbench
```

### CGC
### Real-world applications

## Memory consumption without solving

### Real-world applications

## Code coverage

### CGC
### Real-world applications

## End-to-end fuzzing

### Magma
### Fuzzbench


