KNAPSACK_AST = {
    "mInfo": {
        "finds": [],
        "givens": [],
        "enumGivens": [],
        "enumLettings": [],
        "lettings": [],
        "unnameds": [],
        "strategyQ": {"Auto": {"Interactive": []}},
        "strategyA": {"Auto": {"Interactive": []}},
        "trailCompact": [],
        "nameGenState": [],
        "nbExtraGivens": 0,
        "representations": [],
        "representationsTree": [],
        "originalDomains": [],
        "trailGeneralised": [],
        "trailVerbose": [],
        "trailRewrites": [],
    },
    "mLanguage": {"language": {"Name": "Essence"}, "version": [1, 3]},
    "mStatements": [
        {"Declaration": {"GivenDomainDefnEnum": {"Name": "items"}}},
        {
            "Declaration": {
                "FindOrGiven": [
                    "Given",
                    {"Name": "weight"},
                    {
                        "DomainFunction": [
                            [],
                            [
                                {"SizeAttr_None": []},
                                "PartialityAttr_Total",
                                "JectivityAttr_None",
                            ],
                            {"DomainEnum": [{"Name": "items"}, None, None]},
                            {"DomainInt": [{"TagInt": []}, []]},
                        ]
                    },
                ]
            }
        },
        {
            "Declaration": {
                "FindOrGiven": [
                    "Given",
                    {"Name": "gain"},
                    {
                        "DomainFunction": [
                            [],
                            [
                                {"SizeAttr_None": []},
                                "PartialityAttr_Total",
                                "JectivityAttr_None",
                            ],
                            {"DomainEnum": [{"Name": "items"}, None, None]},
                            {"DomainInt": [{"TagInt": []}, []]},
                        ]
                    },
                ]
            }
        },
        {
            "Declaration": {
                "FindOrGiven": [
                    "Given",
                    {"Name": "capacity"},
                    {"DomainInt": [{"TagInt": []}, []]},
                ]
            }
        },
        {
            "Declaration": {
                "FindOrGiven": [
                    "Find",
                    {"Name": "picked"},
                    {
                        "DomainSet": [
                            [],
                            {"SizeAttr_None": []},
                            {"DomainEnum": [{"Name": "items"}, None, None]},
                        ]
                    },
                ]
            }
        },
        {
            "Objective": [
                "Maximising",
                {
                    "Op": {
                        "MkOpSum": {
                            "Comprehension": [
                                {
                                    "Op": {
                                        "MkOpImage": [
                                            {"Reference": [{"Name": "gain"}, None]},
                                            {"Reference": [{"Name": "i"}, None]},
                                        ]
                                    }
                                },
                                [
                                    {
                                        "Generator": {
                                            "GenInExpr": [
                                                {"Single": {"Name": "i"}},
                                                {
                                                    "Reference": [
                                                        {"Name": "picked"},
                                                        None,
                                                    ]
                                                },
                                            ]
                                        }
                                    }
                                ],
                            ]
                        }
                    }
                },
            ]
        },
        {
            "SuchThat": [
                {
                    "Op": {
                        "MkOpLeq": [
                            {
                                "Op": {
                                    "MkOpSum": {
                                        "Comprehension": [
                                            {
                                                "Op": {
                                                    "MkOpImage": [
                                                        {
                                                            "Reference": [
                                                                {"Name": "weight"},
                                                                None,
                                                            ]
                                                        },
                                                        {
                                                            "Reference": [
                                                                {"Name": "i"},
                                                                None,
                                                            ]
                                                        },
                                                    ]
                                                }
                                            },
                                            [
                                                {
                                                    "Generator": {
                                                        "GenInExpr": [
                                                            {"Single": {"Name": "i"}},
                                                            {
                                                                "Reference": [
                                                                    {"Name": "picked"},
                                                                    None,
                                                                ]
                                                            },
                                                        ]
                                                    }
                                                }
                                            ],
                                        ]
                                    }
                                }
                            },
                            {"Reference": [{"Name": "capacity"}, None]},
                        ]
                    }
                }
            ]
        },
    ],
}
