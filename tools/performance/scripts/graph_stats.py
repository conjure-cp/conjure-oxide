import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
import sys
from scipy.interpolate import interp1d

if len(sys.argv) != 2:
    print("Error: Provide arguments")
    sys.exit(0)


oxideNaive = pd.read_csv('./data/NV_CO/NV_CO.csv') # this is based on where it is called from ugh
nativeO0 = pd.read_csv('./data/O0_CN/O0_CN.csv') # not optimised naive
nativeO2 = pd.read_csv('./data/O2_CN/O2_CN.csv')

#sets needed values for each different measurement, depending on argument
def setVals(column):
    global timeCNO0
    timeCNO0 = nativeO0[column].values #default
    global x
    x = [0, *timeCNO0]
    global timeCNO2
    timeCNO2 = nativeO2[column].values
    global timeCONV
    timeCONV = oxideNaive[column].values

setVals(sys.argv[1])

if (sys.argv[1] != 'solver_nodes'):
    #divide by the default values (CNO0)
    default = np.divide(timeCNO0, timeCNO0)
    speedUpCONV = np.divide(timeCNO0, timeCONV)
    speedUpCNO2 = np.divide(timeCNO0, timeCNO2)

    plt.subplot(2,1,1)
    #extrapolate default line across whole graph
    z = np.polyfit(timeCNO0, default, 1)
    f = np.poly1d(z)
    #plot extrapolated line
    plt.plot((0, max(x)), ((f(0), f(max(x)))), color = '#F67280', label = 'CNO0')
    plt.yscale('log')
    #plot speed up factors on y and time on x
    plt.scatter(timeCNO2,speedUpCNO2, color = '#C06C84', label = 'CNO2', marker = 's')
    plt.scatter(timeCONV,speedUpCONV, color = '#355C7D', label = 'CONV', marker = 'D')

    #titles & labels
    plt.xlabel('Time /s')
    plt.ylabel('Speed-up factor /log')
else:
    tests = nativeO0['test'].values #each test, for x axis
    w, ind = 0.3, np.arange(len(tests)) #width and index
    fig = plt.figure()
    ax = fig.add_subplot(111)

    #set each bar on the barchart at given width
    ax.bar(ind, timeCNO0, w, color = '#F67280', label = 'CNO0')
    ax.bar(ind + w, timeCNO2, w, color = '#C06C84', label = 'CNO2')
    ax.bar(ind + w*2, timeCONV, w, color = '#355C7D', label = 'CONV')

    ax.set_ylabel('Nodes')
    ax.set_xticks(ind+w)
    ax.set_xticklabels(tests,rotation=90) #set x axis

plt.title('Comparing different rewriter modes at ' + sys.argv[1])
plt.legend()
plt.show()