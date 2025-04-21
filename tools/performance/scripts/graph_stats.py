import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from scipy.interpolate import interp1d
# from scipy.optimize import curve_fit

oxideNaive = pd.read_csv('./data/NV_CO/NV_CO.csv') # this is based on where it is called from ugh
nativeO0 = pd.read_csv('./data/O0_CN/O0_CN.csv') # not optimised naive
nativeO2 = pd.read_csv('./data/O2_CN/O2_CN.csv')

#at start,only going to work with speed possibly
rewriteTimeCNO0 = nativeO0['rewriter_time'].values #default
x = [0, *rewriteTimeCNO0]
rewriteTimeCONV = oxideNaive['rewriter_time'].values
rewriteTimeCNO2 = nativeO2['rewriter_time'].values

#divide by the default values (CNO0)
default = np.divide(rewriteTimeCNO0, rewriteTimeCNO0)
speedUpCONV = np.divide(rewriteTimeCNO0, rewriteTimeCONV)
speedUpCNO2 = np.divide(rewriteTimeCNO0, rewriteTimeCNO2)

plt.subplot(2,1,1)
#extrapolate default line across whole graph
z = np.polyfit(rewriteTimeCNO0, default, 1)
f = np.poly1d(z)
#plot extrapolated line
plt.plot((0, max(x)), ((f(0), f(max(x)))), 'r', color = 'green', label = 'CNO0')
plt.yscale('log')
#plot speed up factors on y and time on x
plt.scatter(rewriteTimeCNO2,speedUpCNO2, color = 'blue', label = 'CNO2', marker = 's')
plt.scatter(rewriteTimeCONV,speedUpCONV, color = 'red', label = 'CNO2', marker = 'D')

#titles & labels
plt.xlabel('Time to rewrite /s')
plt.ylabel('Speed-up factor /log')
plt.title('Comparing rewriter time')

#display
plt.legend()
plt.show()