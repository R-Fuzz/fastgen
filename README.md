### Fastgen

Fastgen (forked from Angora) is a continous concolic exection driver with a FIFO seed queue. In the front-end, it uses SymSan to collect the constraints. In the backend, it uses Z3 or JIGSAW as its solver.

### Build

```
./build/build.sh
```


### Dependencies

* Have to point /usr/include/llvm to llvm-6.0
* Have to point /usr/include/llvm-c to llvm-6.0


### Tests result

```
switch2: yes
switch: yes
gep: yes
gep2: yes
alloca: no constraints
bitflip: no. yes with o3
asan: no constraints
bool: no  (not solvable)
call_fn:  yes
call_fn2: yes
call_fn3: yes 
cf1: yes
cf2: yes (with O3)
cf3: no
recursion: no
mini: yes
mini2: yes
shift_and: yes
fstream: yes
stdin_in: getchar() not supported
stat: stat not supported
memcmp:  yes
strcmp:  yes
strcmp2:  yes
loop: yes
infer_type: yes
if_eq: yes
```
