#include "rgd_op.h"
#include "rgd.pb.h"
#include <sys/time.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <google/protobuf/io/zero_copy_stream_impl.h>
#include <fcntl.h>
#include <unistd.h>
using namespace google::protobuf::io;
using namespace rgd;
const uint64_t kUsToS = 1000000;
uint64_t getTimeStamp() {
	struct timeval tv;
	gettimeofday(&tv, NULL);
	return tv.tv_sec * kUsToS + tv.tv_usec;
}

static std::string get_name(uint32_t kind) {
	switch (kind) {
		case rgd::Bool: return "bool";
		case rgd::Constant: return "constant";
		case rgd::Read: return "read";
		case rgd::Concat: return "concat";
		case rgd::Extract: return "extract";

		case rgd::ZExt: return "zext";
		case rgd::SExt: return "sext";

		// Arithmetic
		case rgd::Add:	return "add";
		case rgd::Sub:	return "sub";
		case rgd::Mul:	return "mul";
		case rgd::UDiv:	return "udiv";
		case rgd::SDiv:	return "sdiv";
		case rgd::URem:	return "urem";
		case rgd::SRem:	return "srem";
		case rgd::Neg:	return "neg";

		// Bit
		case rgd::Not: return "not";
		case rgd::And: return "and";
		case rgd::Or: return "or";
		case rgd::Xor: return "xor";
		case rgd::Shl: return "shl";
		case rgd::LShr: return "lshr";
		case rgd::AShr: return "ashr";

		// Compare
		case rgd::Equal: return "equal";
		case rgd::Distinct: return "distinct";
		case rgd::Ult: return "ult";
		case rgd::Ule: return "ule";
		case rgd::Ugt: return "ugt";
		case rgd::Uge: return "uge";
		case rgd::Slt: return "slt";
		case rgd::Sle: return "sle";
		case rgd::Sgt: return "sgt";
		case rgd::Sge: return "sge";

		// Logical
		case rgd::LOr: return "lor";
		case rgd::LAnd: return "land";
		case rgd::LNot: return "lnot";

		// Special
		case rgd::Ite: return "ite";
		case rgd::Memcmp: return "memcmp";
	}
}
static void do_print(const RealAstNode* node) {
	std::cerr << get_name(node->kind()) << "(";
	//std::cerr << req->name() << "(";
	std::cerr << "width=" << node->bits() << ",";
	//std::cerr << " hash=" << req->hash() << ",";
	std::cerr << " label=" << node->label() << ",";
	//std::cerr << " hash=" << req->hash() << ",";
	if (node->kind() == rgd::Bool) {
		std::cerr << node->value();
	}
	if (node->kind() == rgd::Constant) {
		std::cerr << node->value() << ", ";
	//	std::cerr << req->index();
	}
	if (node->kind() == rgd::Memcmp) {
		std::cerr << node->value() << ", ";
	//	std::cerr << req->index();
	}
	if (node->kind() == rgd::Read || node->kind() == rgd::Extract) {
		std::cerr << node->index() << ", ";
	}
	for(int i = 0; i < node->children_size(); i++) {
		do_print(&node->children(i));
		if (i != node->children_size() - 1)
			std::cerr << ", ";
	}
	std::cerr << ")";
}




void printNode(const RealAstNode* node) {
	do_print(node);
	std::cerr << std::endl;
}

void printNode(const AstNode* node) {
  if (node->virt()) 
    std::cerr << node->label();
  else 
	  do_print(&node->payload());
	std::cerr << std::endl;
}

void printTask(const SearchTask* task) {
  for(auto cons : task->constraints())  {
    std::cerr << get_name(cons.comparison()) << std::endl;
    printNode(&cons.left());
    printNode(&cons.right());
  }
}



static bool writeDelimitedTo(
		const google::protobuf::MessageLite& message,
		google::protobuf::io::ZeroCopyOutputStream* rawOutput) {
	// We create a new coded stream for each message.  Don't worry, this is fast.
	google::protobuf::io::CodedOutputStream output(rawOutput);

	// Write the size.
	const int size = message.ByteSizeLong();
	output.WriteVarint32(size);

	uint8_t* buffer = output.GetDirectBufferForNBytesAndAdvance(size);
	if (buffer != NULL) {
		// Optimization:  The message fits in one buffer, so use the faster
		// direct-to-array serialization path.
		message.SerializeWithCachedSizesToArray(buffer);
	} else {
		// Slightly-slower path when the message is multiple buffers.
		message.SerializeWithCachedSizes(&output);
		if (output.HadError()) return false;
	}

	return true;
}

bool saveRequest(
			const google::protobuf::MessageLite& message, 
			const char* path) {
		mode_t mode = S_IRUSR | S_IWUSR;
		int fd = open(path, O_CREAT | O_WRONLY | O_APPEND, mode);
		ZeroCopyOutputStream* rawOutput = new google::protobuf::io::FileOutputStream(fd);
		bool suc = writeDelimitedTo(message,rawOutput);
		delete rawOutput;
		sync();
		close(fd);
		return suc;
}

