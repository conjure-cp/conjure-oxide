import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
import sys
import os
# from scipy.interpolate import interp1d

if len(sys.argv) != 2:
    print("Error: Provide arguments")
    sys.exit(0)

oxideNaive = pd.read_csv('./tools/performance/data/NV_CO/NV_CO.csv') # this is based on where it is called from ugh
nativeO0 = pd.read_csv('./tools/performance/data/O0_CN/O0_CN.csv') # not optimised naive
nativeO2 = pd.read_csv('./tools/performance/data/O2_CN/O2_CN.csv')

timeCNO0 = nativeO0[sys.argv[1]].values
x = [0, *timeCNO0]
timeCNO2 = nativeO2[sys.argv[1]].values
timeCONV = oxideNaive[sys.argv[1]].values

def findZeros(array,zeroIndex):
    index = 0
    for value in array:
        if value == 0:
            zeroIndex.append(index)
        index = index+1
    return zeroIndex

def recordZeros(column,timeCNO0,timeCNO2,timeCONV,zeroIndex):
    tests = nativeO0['test'].values #each test, for x axis
    try:
        os.remove('./tools/performance/data/zeroVals.csv')
    except OSError:
        pass
    csv = open('./tools/performance/data/zeroVals.csv', 'a')
    csv.write("test,value_type,CNO0_value,CNO2_value,CONV_value")
    print(zeroIndex)
    for index in zeroIndex:
        csv.write("\n" + tests[index]+ ',' + column + ',' + str(timeCNO0[index]) + ',' + str(timeCNO2[index]) + ',' + str(timeCONV[index]))
    global resultO0
    resultO0 = np.delete(timeCNO0, zeroIndex)
    global resultO2
    resultO2 = np.delete(timeCNO2, zeroIndex)
    global resultNV
    resultNV = np.delete(timeCONV, zeroIndex)

zeroIndex = []
zeroIndex = findZeros(timeCNO0,zeroIndex)
zeroIndex = findZeros(timeCNO2,zeroIndex)
zeroIndex = findZeros(timeCONV,zeroIndex)
zeroIndex = list(dict.fromkeys(zeroIndex)) #remove duplicate values
recordZeros(sys.argv[1],timeCNO0,timeCNO2,timeCONV,zeroIndex)
timeCNO0 = resultO0.copy()
timeCNO2 = resultO2.copy()
timeCONV = resultNV.copy()

if len(timeCNO0) <= 0:
    print(f"Exiting: All instances had a time of 0.")
    print(f"See ./data/zeroVals.csv for details")
    exit(1)

if (sys.argv[1] != 'solver_nodes'):
    #divide by the default values (CNO0)
    default = np.divide(timeCNO0, timeCNO0,where=(timeCNO0!=0))
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
    fig = plt.figure()
    ax = fig.add_subplot(111)
    plt.subplot()
    ax.scatter(tests, timeCNO0, color = '#F67280', label = 'CNO0', marker = 'X')
    ax.scatter(tests,timeCNO2, color = '#C06C84', label = 'CNO2', marker = 's')
    ax.scatter(tests,timeCONV, color = '#355C7D', label = 'CONV', marker = 'D')
    plt.setp(ax.get_xticklabels(), rotation = 90)

plt.title('Comparing different rewriter modes at ' + sys.argv[1])
plt.legend()
plt.show()