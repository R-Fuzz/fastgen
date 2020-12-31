### Fastgen

Build

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
gep:  yes
gep2: yes
alloca: does not generate seed
bitflip: no
asan: crashes when fuzzing
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
shift_and: not solved
fstream: cpp program
stdin_in: getchar() not supported
stat: stat not supported
strcmp: strcmp  not supported
strcmp2: strcmp not supported
memcmp: memcmp not supported
loop: yes
infer_type: yes
if_eq: yes
```

### TODOs

* add AstNode caching
* Test byte freezing
* Test search from current input

####  Known issues

* Crashes when fuzzing GEP/GEP2
