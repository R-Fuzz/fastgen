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
	return m*60.0 + s

symsanqsymfile=open("symsan_time_qsymbackend")
symccpurefile=open("symcc_time_puretaint")
symccnsfile=open("symcc_time_nosolving")
symqemunsfile=open("symqemu_time_nosolving")
symsannsfile=open("symsan_time_nosolving_new")
symsanpurefile=open("symsan_time_puretaint")
nativefile=open("native_time")
symccfile=open("symcc_time")
symsanfile=open("symsan_time")
symqemufile=open("symqemu_time")
symsanqsymlines = symsanqsymfile.readlines()
symccpurelines = symccpurefile.readlines()
symsanpurelines = symsanpurefile.readlines()
nativelines = nativefile.readlines()
symsannslines=symsannsfile.readlines()
symccnslines=symccnsfile.readlines()
symqemunslines=symqemunsfile.readlines()
symsanlines=symsanfile.readlines()
symcclines=symccfile.readlines()
symqemulines=symqemufile.readlines()
i = 0
Native=[]
SymSanPure=[]
SymCCPure=[]
SymSanNS=[]
SymSanQsym=[]
SymCCNS=[]
SymQEMUNS=[]
SymSan=[]
SymCC=[]
SymQEMU=[]
for line in symccpurelines:
  if line[0] != '*':
    print(i)
    symccpuretime = parse(line.split('\t')[1])
    symsanqsymtime = parse(symsanqsymlines[i].split('\t')[1])
    symsanpuretime = parse(symsanpurelines[i].split('\t')[1])
    symccnstime = parse(symccnslines[i].split('\t')[1])
    symsannstime = parse(symsannslines[i].split('\t')[1])
    symqemunstime = parse(symqemunslines[i].split('\t')[1])
    symcctime = parse(symcclines[i].split('    ')[1])
    symsantime = parse(symsanlines[i].split('\t')[1])
    symqemutime = parse(symqemulines[i].split('\t')[1])
    SymSanPure.append(symsanpuretime)
    SymCCPure.append(symccpuretime)
    SymSanQsym.append(symsanqsymtime)
    SymSanNS.append(symsannstime)
    SymCCNS.append(symccnstime)
    SymQEMUNS.append(symqemunstime)
    SymSan.append(symsantime)
    SymCC.append(symcctime)
    SymQEMU.append(symqemutime)
    if (symccpuretime < 0.0):
      print(symcctime)
  i = i + 1;
for line in nativelines:
	nativetime = parse(line.split('\t')[1])
	Native.append(nativetime)


#average = lambda x: sum(x) / len(x)
#average = lambda x: np.median(x)
average = lambda x: np.percentile(x,50)
a,b,c,d,e,f,g = average(Native), average(SymSanPure), average(SymCCPure), average(SymSanQsym), average(SymSanNS), average(SymCCNS), average(SymQEMUNS)
print("native, symsanpure, symccpure, symsanqsym, symsan, symcc, symqemu")
print(a,b,c,d,e,f,g)
print(1, b/a, c/a, d/a, e/a, f/a, g/a)
columns = [Native, SymSanPure, SymCCPure, SymSanNS, SymSanQsym, SymCCNS, SymQEMUNS, SymSan, SymCC, SymQEMU ]



#print(columns)
fig, ax = plt.subplots(figsize=(30,6))
#box = ax.boxplot(columns, notch=True, patch_artist=False, showmeans=True)
#box = sns.violinplot(columns, showmeans=True)
box = sns.boxplot(data=columns, ax=ax, palette=["orangered", 'lawngreen', 'moccasin', 'lawngreen', 'lawngreen', 'wheat', 'royalblue', 'lawngreen', 'wheat', 'royalblue'])
ax.set_yscale('log')
ax.set_xticklabels(["Native", "SymSan-Taint", "SymCC-Taint", "SymSan-NS", "SymSan-QSYM-NS", "SymCC-NS", "SymQEMU-NS", "SymSan", "SymCC", "SymQEMU"], fontsize=16)
ax.tick_params(axis='both', which='major', labelsize=24)
#plt.ylim([0,300])

plt.savefig("cgc.pdf", bbox_inches="tight")
