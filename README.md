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
### 3.1.3 Real-world applications

## 3.2 Memory consumption without solving

### 3.2.1 Real-world applications

## 3.3 Code coverage

### 3.3.1 CGC
### 3.3.2 Real-world applications

## 3.4 End-to-end fuzzing

### 3.4.1 Magma
### 3.4.2 Fuzzbench


