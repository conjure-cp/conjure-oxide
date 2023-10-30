from typing import Any


def flat_keys_count(
    data: dict[Any, dict | list] | list, blocklist=None
) -> dict[Any, int]:
    """
    Recurse over a dict or list (potentially with nested dicts / lists) and count all dictionary keys
    :param data: a dictionary or list containing dictionaries / lists
    :param blocklist: collection of keys to ignore
    :return: dict in the format of <Key>:<№ of key's occurrences in data>
    """

    ans = {}

    def add_key(key, count=1):
        if (blocklist is None) or (key not in blocklist):
            if key in ans:
                ans[key] += count
            else:
                ans[key] = count

    def recurse_and_add_keys(
        item,
    ):  # Recurse over entry (list or dict) and add its keys to the count
        if isinstance(item, (list, dict)):
            new_keys = flat_keys_count(item)
            for key in new_keys:
                add_key(key, new_keys[key])

    if isinstance(
        data,
        dict,
    ):  # If it's a dict, add its keys and recurse over the values
        for key in data:
            add_key(key)
            recurse_and_add_keys(data[key])
    elif isinstance(data, list):  # If it's a list, recurse over all its elements
        for entry in data:
            recurse_and_add_keys(entry)

    return ans
