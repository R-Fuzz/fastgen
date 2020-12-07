#ifndef TEST_H_
#define TEST_H_
#include <stdint.h>
#include <vector>
#include <map>
#include <memory>
#include <unordered_map>
//function under test
//constraint: 0 = equal, 1 = distinct, 2 = lt, 3 = le, 4 = gt, 5 = ge 
typedef uint64_t(*test_fn_type)(uint64_t*);

class Cons {
public:
	test_fn_type fn;
	uint32_t comparison;

	//map the offset to the idx in inputs_args
	std::unordered_map<uint32_t,uint32_t> local_map;
	// if const {false, const value}, if symbolic {true, index in the inputs}
	std::vector<std::pair<bool, uint64_t>> input_args;
	//map the offset to iv
	std::unordered_map<uint32_t,uint8_t> inputs;
	uint32_t const_num;
};

struct FUT {  
	FUT(): scratch_args(nullptr), max_const_num(0) {}
	~FUT() { if (scratch_args) free(scratch_args); }
	uint32_t num_exprs;
	std::vector<std::shared_ptr<Cons>> constraints;

	// offset and input value
	std::vector<std::pair<uint32_t,uint8_t>> inputs;

	uint64_t start; //start time
	uint32_t max_const_num;
	bool stopped = false;
	int att = 0;
	int num_minimal_optima = 0;
	bool gsol = false;
	bool opti_hit = false;
  std::vector<std::unordered_map<uint32_t,uint8_t>> *rgd_solutions;
	std::unordered_map<uint32_t,uint8_t> *rgd_solution;
	std::unordered_map<uint32_t,uint8_t> *opti_solution;
	uint64_t* scratch_args;
	//void allocate_scratch_args(int size) {scratch_args = (uint8_t*)aligned_alloc(64,size);}
	void finalize() {
	  //aggregate the contraints, fill input_args's index, build global inputs
		std::unordered_map<uint32_t,uint32_t> sym_map;
		uint32_t gidx = 0;
		for (size_t i =0; i< constraints.size(); i++) {
			for (auto itr : constraints[i]->local_map) {
				auto gitr = sym_map.find(itr.first);
				if (gitr == sym_map.end()) {
					gidx = inputs.size();
					sym_map[itr.first] = gidx;
					inputs.push_back(std::make_pair(itr.first,constraints[i]->inputs[itr.first]));
				} else {
					gidx = gitr->second;
				}
				constraints[i]->input_args[itr.second].second = gidx;  //update input_args
			}
		}

		for (size_t i=0; i < constraints.size(); i++) {
			if (max_const_num < constraints[i]->const_num)
				max_const_num = constraints[i]->const_num;
		}

		scratch_args = (uint64_t*)malloc((2 + inputs.size() + max_const_num) * sizeof(uint64_t));
	}

};
#endif
