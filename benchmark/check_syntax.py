import ast
import sys

with open("/home/taplo/getframe/benchmark/run.py") as f:
    ast.parse(f.read())
print("Syntax OK")
