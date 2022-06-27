import matplotlib.pyplot as plt
import numpy as np
from scipy.stats import mannwhitneyu
import pandas as pd
import seaborn as sns
plt.rcParams['font.family'] = ['Times New Roman']

def parse(raw): 
  length=len(raw)
  m = float(raw[0])
  s = float(raw[2:length-2])
  #return m*60.0*1000000.0 + s*1000000.0
  return m*60.0 + s

def parse1(raw): 
	m=float(raw.split(':')[0])
	s=float(raw.split(':')[1])
	#return m*60.0*1000000.0 + s*1000000.0
	return m*60.0 + s


#symsanqsymfile=open("real_symsan_time_qsymbackend")
symccpurefile=open("real_symcc_time_puretaint")
#symccfile=open("real_symcc_time")
symqemufile=open("real_symqemu_time")
#symqemufile=open("all_symqemu")
#symsanfile=open("real_symsan_time")
symsanpurefile=open("real_symsan_time_puretaint")
nativefile=open("real_native_time")
#symsanqsymlines = symsanqsymfile.readlines()
symccpurelines = symccpurefile.readlines()
symsanpurelines = symsanpurefile.readlines()
nativelines = nativefile.readlines()
#symsanlines=symsanfile.readlines()
#symcclines=symccfile.readlines()
symqemulines=symqemufile.readlines()
Native=[]
SymSanPure=[]
SymCCPure=[]
SymQEMU=[]
for line in symqemulines:
  print(line)
  symqemutime = parse1(line.split(' ')[0])
  if (symqemutime!=0.0):
    SymQEMU.append(symqemutime)

for line in nativelines:
  nativetime = parse(line.split('\t')[1])
  if (nativetime!=0.0):
    Native.append(nativetime)

for line in symsanpurelines:
  symsanpuretime = parse(line.split('\t')[1])
  if (symsanpuretime!=0.0):
    SymSanPure.append(symsanpuretime)

for line in symccpurelines:
  symccpuretime = parse(line.split('\t')[1])
  if (symccpuretime!=0.0):
    SymCCPure.append(symccpuretime)

SymCC=[]
symccfile=open("symcc_time_all")
for line in symccfile.readlines():
  symcctime=parse1(line.split(' ')[0])
  if (symcctime!=0.0):
    SymCC.append(symcctime)

SymSan=[]
symsanfile=open("symsan_time_all")
for line in symsanfile.readlines():
  print(line.split('\t')[-1])
  symsantime=parse(line.split('\t')[-1])
  print(symsantime)
  if (symsantime!=0.0):
    SymSan.append(symsantime)

SymSanQsym=[]
symsanqsymfile=open("symsanqsym_time_all")
for line in symsanqsymfile.readlines():
  symsanqsymtime=parse1(line.split(' ')[0])
  if (symsanqsymtime!=0.0):
    SymSanQsym.append(symsanqsymtime)



SymSanSolve=[]
fp=open("real_symsan_time_solving")
lines=fp.readlines()
for line in lines:
  lineitems = line.split(' ')
  time1=float(lineitems[-1])/1000000.0
  if(time1>0.0):
    SymSanSolve.append(time1)


SymCCSolve=[]
fp=open("real_symcc_time_solving")
lines=fp.readlines()
for line in lines:
  mytime = line.split(' ')[0]
  mytimemin=mytime.split(':')[0]
  mytimesec=mytime.split(':')[1]
  #time2=float(mytimemin)*60*1000000.0 + float(mytimesec)*1000000.0
  time2=float(mytimemin)*60 + float(mytimesec)
  if(time2>0.0):
    SymCCSolve.append(time2)



#average = lambda x: sum(x) / len(x)
average = lambda x: float(np.median(x))
a,b,c,d,e,f,g,h,i = average(Native), average(SymSanPure), average(SymCCPure), average(SymSanQsym), average(SymSan), average(SymCC), average(SymQEMU), average(SymSanSolve), average(SymCCSolve)
print("native, symsanpure, symccpure, symsanqsym, symsan, symcc, symqemu, symsansolve, symccsolve")
print(a,b,c,d,e,f,g,h,i)
print(1, b/a, c/a, d/a, e/a, f/a, g/a)
print(f/e, g/e)
columns = [Native, SymSanPure, SymCCPure, SymSan, SymSanQsym, SymCC, SymQEMU, SymSanSolve, SymCCSolve]



#print(columns)
fig, ax = plt.subplots(figsize=(30,6))
#box = ax.boxplot(columns, notch=True, patch_artist=False, showmeans=True)
#box = sns.violinplot(columns, showmeans=True)
box = sns.boxplot(data=columns, ax=ax, palette=["orangered", 'lawngreen', 'moccasin', 'lawngreen', 'lawngreen', 'wheat', 'royalblue', 'lawngreen', 'wheat'])
ax.set_yscale('log')
ax.set_xticklabels(["Native", "SymSan-Taint", "SymCC-Taint", "SymSan-NS", "SymSan-QSYM-NS", "SymCC-NS", "SymQEMU-NS", 'SymSan', 'SymCC'], fontsize=16)
ax.tick_params(axis='both', which='major', labelsize=24)
#plt.ylim([0,300])

plt.savefig("real.pdf", bbox_inches="tight")
