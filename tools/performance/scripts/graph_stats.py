import pandas as pd
import matplotlib.pyplot as plt
from matplotlib.ticker import ScalarFormatter
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

column = sys.argv[1]

timeCNO0 = nativeO0[column].values
timeCNO2 = nativeO2[column].values
timeCONV = oxideNaive[column].values
allTimes = [timeCNO0, timeCNO2, timeCONV]
values =  np.append(timeCNO0, timeCNO2)
values = np.append(values,timeCONV)
x = [0, *values]

def findZeros(array,zeroIndex):
    index = 0
    for value in array:
        if value == 0.0:
            zeroIndex.append(index)
        index = index+1
    return zeroIndex

def recordZeros():
    tests = nativeO0['test'].values #each test, for x axis
    try:
        os.remove('./tools/performance/data/zeroVals.csv')
    except OSError:
        pass
    csv = open('./tools/performance/data/zeroVals.csv', 'a')
    csv.write("test,value_type,CNO0_value,CNO2_value,CONV_value")
    print(zeroIndex)
    for index in zeroIndex:
        csv.write("\n" + tests[index]+ ',' + column + ',' + str(allTimes[0][index]) + ',' + str(allTimes[1][index]) + ',' + str(allTimes[2][index]))
    resultO0 = np.delete(allTimes[0], zeroIndex)
    resultO2 = np.delete(allTimes[1], zeroIndex)
    resultNV = np.delete(allTimes[2], zeroIndex)
    return [resultO0,resultO2,resultNV]

zeroIndex = []
for item in allTimes:
    zeroIndex = findZeros(item,zeroIndex)
zeroIndex = list(dict.fromkeys(zeroIndex)) #remove duplicate values
allTimes = recordZeros()

if len(allTimes[0]) <= 0:
    print(f"Exiting: All instances had a time of 0.")
    print(f"See ./data/zeroVals.csv for details")
    sys.exit(0)

if (column != 'solver_nodes'):
    #divide by the default values (CNO0)
    default = np.divide(allTimes[0], allTimes[0])
    speedUpCONV = np.divide(allTimes[0], allTimes[2])
    speedUpCNO2 = np.divide(allTimes[0], allTimes[1])

    fig, ax = plt.subplots()
    #extrapolate default line across whole graph
    z = np.polyfit(allTimes[0], default, 1)
    f = np.poly1d(z)
    #plot extrapolated line
    ax.plot((0, max(x)), ((f(0), f(max(x)))), color = '#F67280', label = 'CNO0')

    ax.set_yscale('log')
    
    #plot speed up factors on y and time on x
    ax.scatter(allTimes[1],speedUpCNO2, color = '#C06C84', label = 'CNO2', marker = 's')
    ax.scatter(allTimes[2],speedUpCONV, color = '#355C7D', label = 'CONV', marker = 'D')

    #titles & labels
    ax.yaxis.set_major_formatter(ScalarFormatter())
    plt.xlabel('Time /s')
    plt.ylabel('Speed-up factor /log')
else:
    testsO0 = nativeO0['test'].values #each test, for x axis
    testsO2 = nativeO2['test'].values
    testsNV = oxideNaive['test'].values
    fig = plt.figure()
    ax = fig.add_subplot(111)
    plt.subplot()
    ax.scatter(testsO2,allTimes[1], color = '#C06C84', label = 'CNO2', marker = 's')
    ax.scatter(testsNV,allTimes[2], color = '#355C7D', label = 'CONV', marker = 'D')
    ax.scatter(testsO0, allTimes[0], color = '#F67280', label = 'CNO0', marker = 'X')
    plt.setp(ax.get_xticklabels(), rotation = 90)
    plt.ylabel("Number of Nodes")

plt.title('Comparing different rewriter modes at ' + column)
plt.legend()
plt.show()