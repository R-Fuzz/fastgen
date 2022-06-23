# 1. Fastgen

Fastgen (forked from Angora) is a continous concolic exection driver with a FIFO seed queue. In the front-end, it uses SymSan to collect the constraints. In the backend, it uses Z3 or JIGSAW as its solver.

# 2. Installation

```
cd usenix
docker build -t usenix .
```

# 3. Experiments Results Reproduction


## 3.1 Excution time without solving 

### 3.1.1 nbench

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
cd /src/cgc_programs/build_symcc
./run_symcc.sh
```

4. Run SymQEMU

```
cd /src/cgc_programs/build
/src/cgc_programs/build/run_symqemu.sh
```

### 3.1.3 Real-world applications

1. Run Native

```
/src/build-programs-native/run_native_time.sh /out/real_seeds/objdump_reduced objdump -D
```

2. Run SymSan

```
cd /src/build-programs-symsannosolve
./run_symsan_time.sh /out/real_seeds/objdump_reduced objdump -D
```

3. Run SymCC

```
/src/build-programs-symcc/run_symcc_time.sh /out/real_seeds/objdump_reduced objdump -D
```

4. Run SymQEMU

```
/src/build-programs-native/run_symqemu_time.sh /out/real_seeds/objdump_reduced objdump -D
```

## 3.2 Memory consumption without solving

### 3.2.1 Real-world applications

1. Run Native

```
/src/build-programs-native/run_native_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

2. Run SymSan

```
/src/build-programs-symsannosolve/run_symsan_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

3. Run SymCC

```
/src/build-programs-symcc/run_symcc_mem.sh /out/real_seeds/objdump_reduced objdump -D
```

## 3.3 Code coverage

### 3.3.1 CGC
### 3.3.2 Real-world applications

## 3.4 End-to-end fuzzing

### 3.4.1 Magma
### 3.4.2 Fuzzbench


