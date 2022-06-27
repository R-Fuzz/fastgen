import matplotlib.pyplot as plt
plt.rcParams['font.family'] = ['Times New Roman']
import numpy as np
from scipy.stats import mannwhitneyu
import pandas as pd
import seaborn as sns
average = lambda x: np.median(x)
#average = lambda x: sum(x) / len(x)
def parse(raw): 
    length=len(raw)
    m = float(raw[0])
    s = float(raw[2:length-2])
    return m*60.0 + s
symccfile=open("symcc_time_all", 'r').readlines()
symsanfile = open("symsan_mem_all", 'r').readlines()
symsanqsymfile = open("symsanqsym_mem_all", 'r').readlines()
nativefile = open("native_mem_all", 'r').readlines()
i = 1
SymCC=[]
SymSan=[]
SymSanQsym=[]
SymQEMU=[]
Native=[]
for line in symccfile:
    SymCC.append(int(line.split(' ')[1]))

for line in symsanfile:
    SymSan.append(int(line.split(' ')[1]))

for line in nativefile:
    Native.append(int(line.split(' ')[1]))

for line in symsanqsymfile:
    SymSanQsym.append(int(line.split(' ')[1]))

a,b,c,d = average(Native), average(SymSan), average(SymSanQsym), average(SymCC)
print("native, symsan, symsanqsym, symcc")
print(a,b,c,d)
print(d/b)
print(1, b/a, c/a,d/a)

columns = [Native, SymSan, SymSanQsym, SymCC]
plt.ylabel("Maximum Resident Size(KB)", fontsize=16)
#print(columns)
fig, ax = plt.subplots(figsize=(10,6))
#box = ax.boxplot(columns, notch=True, patch_artist=False, showmeans=True)
#box = sns.violinplot(columns, showmeans=True)
box = sns.boxenplot(data=columns, ax=ax, palette=["orangered", 'lawngreen', 'lawngreen', 'wheat'])
ax.set_yscale('log')
ax.set_xticklabels(["Native", "SymSan", "SymSan-QSYM", "SymCC"], fontsize=16)
ax.tick_params(axis='both', which='major', labelsize=24)
#plt.ylim([0,300])

plt.savefig("mem.pdf", bbox_inches="tight")
