## REQUIREMENTS

To download requirement dependencies, follow the folling steps:

1. Make sure you are in the correct repo directory
```bash
cd ./tools/ci-test-benchmarks
```

2. Create virtual environment
```bash
python -m venv env
```

3. Activate virtual environment
```bash
# for Windows 10/11
.\env\Scripts\activate

# for macOS/Linux
source env/bin/activate
```

4. Install required packages
```bash
pip install -r requirements.txt
```

## EXECUTE
```bash
python app.py
```

## USAGE

Using conjure native repo submodule, to update this, run the following command:

```bash
git submodule update --init --recursive
```

