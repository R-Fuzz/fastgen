#include "llvm/ADT/APFloat.h"
#include "llvm/ADT/STLExtras.h"
#include "llvm/IR/BasicBlock.h"
#include "llvm/IR/Constants.h"
#include "llvm/IR/DerivedTypes.h"
#include "llvm/IR/Function.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Type.h"
#include "llvm/IR/Verifier.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Transforms/InstCombine/InstCombine.h"
#include "llvm/Transforms/Scalar.h"
#include "llvm/Transforms/Scalar/GVN.h"
#include "rgd.pb.h"
#include "rgd_op.h"
#include "util.h"
#include <iostream>
#include <unordered_map>

using namespace llvm;
using namespace rgd;

//Generate code for a AST node.
//There should be no relational (Equal, Distinct, Ult, Ule, Ugt, Uge, Sle, Sle, Sgt, Sge) operators in the node
//Builder: LLVM IR builder
//localmap:  for each Constant/Read node (leaf node), find its position in the argument
llvm::Value* codegen(llvm::IRBuilder<> &Builder,
		const JitRequest* request,
		std::unordered_map<uint32_t,uint32_t> &local_map, llvm::Value* arg,
		std::unordered_map<uint32_t, llvm::Value*> &value_cache) {

	llvm::Value* ret = nullptr;

	auto itr = value_cache.find(request->label());
	if (request->label() != 0
			&& itr != value_cache.end()) {
		return itr->second;
	}

	switch (request->kind()) {
		case rgd::Bool: {
			// getTrue is actually 1 bit integer 1
			if(request->boolvalue())
				ret = llvm::ConstantInt::getTrue(Builder.getContext());
			else
				ret = llvm::ConstantInt::getFalse(Builder.getContext());
			break;
		}
		case rgd::Constant: {
			uint32_t start = request->index();
			uint32_t length = request->bits()/8;

			llvm::Value* idx[1];
			idx[0] = llvm::ConstantInt::get(Builder.getInt32Ty(),start);
			ret = Builder.CreateLoad(Builder.CreateGEP(arg,idx));
			ret = Builder.CreateTrunc(ret, llvm::Type::getIntNTy(Builder.getContext(),request->bits()));
			break;
		}

		case rgd::Read: {
			uint32_t start = local_map[request->index()];
			size_t length = request->bits()/8;
			llvm::Value* idx[1];
			idx[0] = llvm::ConstantInt::get(Builder.getInt32Ty(),start);
			ret = Builder.CreateLoad(Builder.CreateGEP(arg,idx));
			for(uint32_t k = 1; k < length; k++) {
				idx[0] = llvm::ConstantInt::get(Builder.getInt32Ty(),start+k);
				llvm::Value* tmp = Builder.CreateLoad(Builder.CreateGEP(arg,idx));
				tmp = Builder.CreateShl(tmp, 8 * k);
				ret =Builder.CreateOr(ret,tmp);
			}
			ret = Builder.CreateTrunc(ret, llvm::Type::getIntNTy(Builder.getContext(),request->bits()));
			break;
		}
		case rgd::Concat: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			uint32_t bits =  rc1->bits() + rc2->bits(); 
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateOr(
					Builder.CreateShl(
						Builder.CreateZExt(c2,llvm::Type::getIntNTy(Builder.getContext(),bits)),
						rc1->bits()),
					Builder.CreateZExt(c1, llvm::Type::getIntNTy(Builder.getContext(), bits)));
			break;
		}
		case rgd::Extract: {
			const JitRequest* rc = &request->children(0);
			llvm::Value* c = codegen(Builder,rc, local_map, arg, value_cache);
			ret = Builder.CreateTrunc(
					Builder.CreateLShr(c, request->index()),
					llvm::Type::getIntNTy(Builder.getContext(), request->bits()));
			break;
		}
		case rgd::ZExt: {
			const JitRequest* rc = &request->children(0);
			llvm::Value* c = codegen(Builder,rc, local_map, arg, value_cache);
			ret = Builder.CreateZExtOrTrunc(c, llvm::Type::getIntNTy(Builder.getContext(), request->bits()));
			break;
		}
		case rgd::SExt: {
			const JitRequest* rc = &request->children(0);
			llvm::Value* c = codegen(Builder,rc,local_map, arg, value_cache);
			ret = Builder.CreateSExt(c, llvm::Type::getIntNTy(Builder.getContext(), request->bits()));
			break;
		}
		case rgd::Add: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateAdd(c1, c2);
			break;
		}
		case rgd::Sub: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateSub(c1, c2);
			break;
		}
		case rgd::Mul: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateMul(c1, c2);
			break;
		}
		case rgd::UDiv: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			llvm::Value* VA0 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 0);
			llvm::Value* VA1 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 1);
			llvm::Value* cond = Builder.CreateICmpEQ(c2,VA0);
			llvm::Value* divisor = Builder.CreateSelect(cond,VA1,c2);
			ret = Builder.CreateUDiv(c1, divisor);
			break;
		}
		case rgd::SDiv: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			llvm::Value* VA0 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 0);
			llvm::Value* VA1 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 1);
			llvm::Value* cond = Builder.CreateICmpEQ(c2,VA0);
			llvm::Value* divisor = Builder.CreateSelect(cond,VA1,c2);
			ret = Builder.CreateSDiv(c1, divisor);
			break;
		}
		case rgd::URem: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			llvm::Value* VA0 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 0);
			llvm::Value* VA1 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 1);
			llvm::Value* cond = Builder.CreateICmpEQ(c2,VA0);
			llvm::Value* divisor = Builder.CreateSelect(cond,VA1,c2);
			ret = Builder.CreateURem(c1, divisor);
			break;
		}
		case rgd::SRem: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			llvm::Value* VA0 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 0);
			llvm::Value* VA1 = llvm::ConstantInt::get(llvm::Type::getIntNTy(Builder.getContext(), request->bits()), 1);
			llvm::Value* cond = Builder.CreateICmpEQ(c2,VA0);
			llvm::Value* divisor = Builder.CreateSelect(cond,VA1,c2);
			ret = Builder.CreateSRem(c1, divisor);
			break;
		}
		case rgd::Neg: {
			const JitRequest* rc = &request->children(0);
			llvm::Value* c = codegen(Builder,rc, local_map, arg, value_cache);
			ret = Builder.CreateNeg(c);
			break;
		}
		case rgd::Not: {
			const JitRequest* rc = &request->children(0);
			llvm::Value* c = codegen(Builder,rc, local_map, arg, value_cache);
			ret = Builder.CreateNot(c);
			break;
		}
		case rgd::And: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateAnd(c1, c2);
			break;
		}
		case rgd::Or: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateOr(c1, c2);
			break;
		}
		case rgd::Xor: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateXor(c1, c2);
			break;
		}
		case rgd::Shl: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateShl(c1, c2);
			break;
		}
		case rgd::LShr: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateLShr(c1, c2);
			break;
		}
		case rgd::AShr: {
			const JitRequest* rc1 = &request->children(0);
			const JitRequest* rc2 = &request->children(1);
			llvm::Value* c1 = codegen(Builder,rc1, local_map, arg, value_cache);
			llvm::Value* c2 = codegen(Builder,rc2, local_map, arg, value_cache);
			ret = Builder.CreateAShr(c1, c2);
			break;
		}
		// all the following ICmp expressions should be top level
		case rgd::Equal: {
			assert(false && "Equal expression");
			break;
		}
		case rgd::Distinct: {
			assert(false && "Distinct expression");
			break;
		}
		//for all relation comparison, we extend to 64-bit
		case rgd::Ult: {
			assert(false && "Ult expression");
			break;
		}
		case rgd::Ule: {
			assert(false && "Ule expression");
			break;
		}
		case rgd::Ugt: {
			assert(false && "Ugt expression");
			break;
		}
		case rgd::Uge: {
			assert(false && "Uge expression");
			break;
		}
		case rgd::Slt: {
			assert(false && "Slt expression");
			break;
		}
		case rgd::Sle: {
			assert(false && "Sle expression");
			break;
		}
		case rgd::Sgt: {
			assert(false && "Sgt expression");
			break;
		}
		case rgd::Sge: {
			assert(false && "Sge expression");
			break;
		}
		// this should never happen!
		case rgd::LOr: {
			assert(false && "LOr expression");
			break;
		}
		case rgd::LAnd: {
			assert(false && "LAnd expression");
			break;
		}
		case rgd::LNot: {
			assert(false && "LNot expression");
			break;
		}
		case rgd::Ite: {
			assert(false && "ITE expression");
			break;
			// don't handle ITE for now, doesn't work with GD
#if DEUBG
			std::cerr << "ITE expr codegen" << std::endl;
#endif
#if 0
			const JitRequest* rcond = &request->children(0);
			const JitRequest* rtv = &request->children(1);
			const JitRequest* rfv = &request->children(2);
			llvm::Value* cond = codegen(rcond, local_map, arg, value_cache);
			llvm::Value* tv = codegen(rtv, local_map, arg, value_cache);
			llvm::Value* fv = codegen(rfv, local_map, arg, value_cache);
			ret = Builder.CreateSelect(cond, tv, fv);
#endif
			break;}
		default:
			std::cerr << "WARNING: unhandled expr: ";
			printExpression(request);
			break;
	}

	if (ret && request->label()!=0) {
		value_cache.insert({request->label(), ret});
	}

	return ret; 
}

