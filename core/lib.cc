#include <stdio.h>
extern "C" {
  void print_buffer(const unsigned char* input, unsigned int input_length) {
    for (int i=0; i<input_length;i++)
      printf("%d,",input[i]);
    printf("\n");
  }
}
