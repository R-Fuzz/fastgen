#include "rgd_op.h"
#include "rgd.pb.h"
#include <sys/time.h>
#include <sys/types.h>
#include <sys/mman.h>
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
static void do_print(const AstNode* node) {
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




void printNode(const AstNode* node) {
	do_print(node);
	std::cerr << std::endl;
}


void printTask(const SearchTask* task) {
  printf("fid is %u\n",task->fid());
  for(auto cons : task->constraints())  {
    printNode(&cons.node());
    for (auto amap : cons.meta().map()) {
      printf("k is %u, v is %u\n", amap.k(), amap.v());
    }
    for (auto aarg : cons.meta().args()) {
      printf("isinput is %u, v is %lu\n", aarg.isinput(), aarg.v());
    }
    for (auto ainput : cons.meta().inputs()) {
      printf("offset is %u, iv is %u\n", ainput.offset(), ainput.iv());
    }
    printf("num_const is %u\n", cons.meta().const_num());
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

bool readDelimitedFrom(
		google::protobuf::io::ZeroCopyInputStream* rawInput,
		google::protobuf::MessageLite* message) {
	// We create a new coded stream for each message.  Don't worry, this is fast,
	// and it makes sure the 64MB total size limit is imposed per-message rather
	// than on the whole stream.  (See the CodedInputStream interface for more
	// info on this limit.)
	google::protobuf::io::CodedInputStream input(rawInput);

  input.SetRecursionLimit(10000);

	// Read the size.
	uint32_t size;
	if (!input.ReadVarint32(&size)) return false;

	// Tell the stream not to read beyond that size.
	google::protobuf::io::CodedInputStream::Limit limit =
		input.PushLimit(size);

	// Parse the message.
	if (!message->MergeFromCodedStream(&input)) return false;
	if (!input.ConsumedEntireMessage()) return false;

	// Release the limit.
	input.PopLimit(limit);

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

uint32_t load_input(std::string input_file, unsigned char* input) {
  int fdin;
  struct stat statbuf;
  void *src;
  if ((fdin = open (input_file.c_str(), O_RDONLY)) < 0)
  {
    fprintf(stderr, "cannot open input file %s!\n", strerror(errno));
    return 0;
  }

  if (fstat (fdin,&statbuf) < 0)
  {
    //assert (false && "fstat error");
    fprintf(stderr, "cannot stat file %s!\n", strerror(errno));
    close(fdin);
    return 0;
  }

  if ((src = mmap (0, statbuf.st_size, PROT_READ, MAP_SHARED, fdin, 0))
      == (caddr_t) -1) {
    fprintf(stderr, "cannot map file %s!\n", strerror(errno));
    close(fdin);
    return 0;
  }

	memcpy (input, src, statbuf.st_size);
	munmap(src,statbuf.st_size);
  close(fdin);
  return statbuf.st_size;
}

void generate_input(std::unordered_map<uint32_t,uint8_t> &sol, std::string taint_file, std::string outputDir, uint32_t fid) {
	char path[PATH_MAX];
	//std::string __output_dir = "/home/cju/e2e_jigsaw/size_src/kirenenko-out-0/queue";
	//std::string __output_dir = "/home/cju/tmp";
	std::string old_string = std::to_string(fid);
	std::string output_file = outputDir + "/" + 
		"id-" + std::string(8-old_string.size(),'0') + old_string;
	//std::string input_file = std::string(__output_dir) + "/" + taint_file;
	std::string input_file =  taint_file;
	struct stat statbuf;
	void *src, *dst;
	int fdout, fdin;
	int mode = 0x777;

	if ((fdin = open (input_file.c_str(), O_RDONLY)) < 0)
	{
		//assert(false && "can't open file for reading");
		fprintf(stderr, "cannot open input file %s!\n", strerror(errno));
		goto fail;
	}

	if (fstat (fdin,&statbuf) < 0)
	{
		//assert (false && "fstat error");
		fprintf(stderr, "cannot stat file %s!\n", strerror(errno));
		goto fail1;
	}	

	if ((src = mmap (0, statbuf.st_size, PROT_READ, MAP_SHARED, fdin, 0))
			== (caddr_t) -1) {
		fprintf(stderr, "cannot map file %s!\n", strerror(errno));
		goto fail1;
	}


	if ((fdout = open (output_file.c_str(), O_RDWR | O_CREAT | O_TRUNC, mode)) < 0)//edited here
	{
		fprintf(stderr, "cannot open outputfile %s!\n", strerror(errno));
		goto fail2;
	}

	dst = malloc(statbuf.st_size);
  

	/* this copies the input file to the output file */
	memcpy (dst, src, statbuf.st_size);
	//memset(dst,0,sizeof(dst));
  for (auto it=sol.begin();it!=sol.end();it++) {
		((uint8_t*)dst)[it->first] = it->second;
		printf("generate_input index is %u and value is %x and original value is %x\n", it->first,(uint32_t)it->second, ((uint8_t*)src)[it->first]);
	}


	if (write(fdout, dst, statbuf.st_size) < 0) {
		fprintf(stderr, "write output error: %s!\n", strerror(errno));
		goto fail3;
	}
fail3:
	close(fdout);
	free(dst);
fail2:
	munmap(src,statbuf.st_size);
fail1:
	close(fdin);
fail:
	return;
}




