typedef uint32_t dfsan_label;

struct dfsan_label_info {
  dfsan_label l1;
  dfsan_label l2;
  uint64_t op1;
  uint64_t op2;
  uint16_t op;
  uint16_t size;
  uint32_t hash;
  uint32_t tree_size;
  uint32_t depth;
  uint8_t flags;
  uint8_t padding[7];
} __attribute__((packed));

