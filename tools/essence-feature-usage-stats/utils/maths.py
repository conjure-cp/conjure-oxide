def map_range(x, in_min, in_max, out_min, out_max):
    """
    Maps x between in_min, in_max to range between out_min and out_max
    :param x: value to map
    :param in_min: min value of x
    :param in_max: max value of x
    :param out_min: min value of output range
    :param out_max: max value of output range
    :return: mapped value
    """
    if in_min == in_max:
        return out_min + (out_max - out_min) / 2
    return (x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min


def clamp(x, minn, maxn):
    return min(max(x, minn), maxn)
