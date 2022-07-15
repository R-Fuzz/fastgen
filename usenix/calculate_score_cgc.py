from pathlib import Path

def get_score(bin_name):
  symcc_bucket= [0] * 65536
  symsan_bucket= [0] * 65536
  #pathlist = Path("trace_symcc_nolinear_" + bin_name).rglob('*')
  pathlist = Path(bin_name + "_symcc_trace").rglob('*')
  for path in pathlist:
    path_in_str = str(path)
    fp=open(path_in_str)
    lines=fp.readlines()
    for line in lines:
      lineitems = line.split(':')
      #print(int(lineitems[0]))
      symcc_bucket[int(lineitems[0])] = symcc_bucket[int(lineitems[0])]+ 1

  pathlist = Path(bin_name + "_symsan_trace").rglob('*')
  for path in pathlist:
    path_in_str = str(path)
    fp=open(path_in_str)
    lines=fp.readlines()
    for line in lines:
      lineitems = line.split(':')
      #print(int(lineitems[0]))
      symsan_bucket[int(lineitems[0])] = symsan_bucket[int(lineitems[0])]+ 1
  union = 0
  join = 0
  symsan_unique = 0
  symcc_unique = 0

  for x in range(65536):
    if (symsan_bucket[x] != 0):
      if (symcc_bucket[x] != 0):
        join = join + 1  
        union = union + 1  
      else:
        union = union + 1  
        symsan_unique = symsan_unique + 1
    else:
      if (symcc_bucket[x] != 0):
        union = union + 1
        symcc_unique = symcc_unique + 1
  if (union-join != 0):
    result = float(symsan_unique - symcc_unique) / float(union - join) 
    print(result)
    return result
  else:
    return 0.0

t = [ [0.0]*17 for i in range(6)]
symccbetter=0
symsanbetter=0
print(t)
fp=open("/src/cgc_list")
lines=fp.readlines()
count = 0
for line in lines:
  print(line.strip())
  x = int(count / 17)
  y = int(count % 17)
  count=count+1;
  t[x][y] = get_score(line.strip())
  if (t[x][y] > 0.0):
    symsanbetter = symsanbetter + 1
  if (t[x][y] < 0.0):
    symccbetter = symccbetter + 1
print(t)
print(symsanbetter)
print(symccbetter)
