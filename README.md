# 1. Fastgen

Fastgen (forked from Angora) is a continous concolic exection driver with a FIFO seed queue. In the front-end, it uses SymSan to collect the constraints. In the backend, it uses Z3 or JIGSAW as its solver.

# 2. Installation

```
cd usenix
docker build -t usenix .
```

# 3. Experiments Results Reproduction


## 3.1 Excution time without solving 

Run docker image interactively and disable SymCC solving

```
docker run -it --ulimit core=0 usenix /bin/bash
cd /symcc
patch -p1 < /src/symcc_nosolve.patch
cd build
rm -rf libSymbolize.so
ninja
```


### 3.1.1 nbench

1. Run Native

```
cd /src/nbench_native
./nbench
```

2. Run SymSan

```
cd /src/nbench_symsan
./nbench
```

3. Run SymCC

```
cd /src/nbench_symcc
SYMCC_NO_SYMBOLIC_INPUT=1 ./nbench
```

### 3.1.2 CGC

1. Run Native

```
cd /src/cgc_programs/build
./run_native.sh
```

2. Run SymSan

```
cd /src/cgc_programs/build_symsannosolve
./run_symsan.sh
```

3. Run SymCC

```
cd /src/cgc_programs/
build_symcc
./run_symcc.sh
```

4. Run SymQEMU

```
cd /src/cgc_programs/build
/src/cgc_programs/build/run_symqemu.sh
```

### 3.1.3 Real-world applications

1. Run Native (objdump)

```
cd /src/build-programs-native
./run_native_time.sh /out/real_seeds/objdump_reduced objdump -D
```

2. Run SymSan (objdump)

```
cd /src/build-programs-symsannosolve
./run_symsan_time.sh /out/real_seeds/objdump_reduced objdump -D
```

3. Run SymCC (objdump)

```
cd /src/build-programs-symcc
./run_symcc_time.sh /out/real_seeds/objdump_reduced objdump -D
```

4. Run SymQEMU (objdump)

```
cd /src/build-programs-native
./run_symqemu_time.sh /out/real_seeds/objdump_reduced objdump -D
```

## 3.2 Memory consumption without solving

### 3.2.1 Real-world applications

1. Run Native

```
cd /src/build-programs-native
./run_native_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

2. Run SymSan

```
cd /src/build-programs-symsannosolve
./run_symsan_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

3. Run SymCC

```
cd /src/build-programs-symcc
./run_symcc_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

## 3.3 Code coverage

Re-enable SymCC's solving

```
cd /symcc
git reset --hard
cd build
rm -rf libSymbolize.so
ninja
```

### 3.3.1 CGC

1. Run SymSan

```
cd /src/cgc_programs/build_symsan
./run_symsan.sh
```

2. Run SymCC

```
cd /src/cgc_programs/build_symcc
./run_symcc.sh
```

### 3.3.2 Real-world applications

1. Run SymCC (objdump)

```
cd /src/build-programs-symcc
./run_symcc_time.sh /out/real_seeds/objdump_reduced objdump -D
```

2. Run SymSan

Patch SymSan with CE testing

```
cd /symsan && patch -p1 < /src/symsan_cov.patch
./build/build.sh
cp target/release/fastgen /src/build-programs
```

Run objdump (560 is the number of inputs, see paper's Table 3 for all other programs)

```
cd /src/build-programs
./fuzzer.sh 560 objdump -D
```



## 3.4 End-to-end fuzzing

### 3.4.1 Magma

Run the forked magma with SymSan added

```
https://github.com/chenju2k6/magma
```

### 3.4.2 Fuzzbench

The below link contains the SymSan patch we sent to Google's Fuzzbench team

```
https://drive.google.com/file/d/1fQTCzWJJkzc6QK1q-m7aIfyQWGtp85bJ/view?usp=sharing
```
