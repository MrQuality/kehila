"""Pure SP-003 goldens and actual Rust domain-codec checks for shared verification."""
import sys
from build import build
from cleanup_errors import run_main
from run import Cases, pure_cases, require


def main():
    with build() as (binary, output):
        print(output, end="")
        cases = Cases(binary)
        pure_cases(cases)
        require(all(record["status"] == "PASS" for record in cases.records), "SP-003 pure codec check failed")


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_SP003_CHECK'))
