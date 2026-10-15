"""Pure SP-003 goldens and actual Rust domain-codec checks for shared verification."""
import sys
from build import build
from cleanup_errors import run_main
from run import Cases, pure_cases, require


def report_failures(records):
    """Print each recorded failure as one line of at most 4096 UTF-8 bytes."""
    for record in records:
        if record['status'] != 'FAIL':
            continue
        message = 'CASE FAILED (' + record['case'] + '): ' + record['error']
        message = message.replace('\r', '\\r').replace('\n', '\\n')
        bounded = message.encode('utf-8', errors='replace')[:4096].decode('utf-8', errors='ignore')
        print(bounded, file=sys.stderr, flush=True)


def main():
    with build() as (binary, output):
        print(output, end="")
        cases = Cases(binary)
        pure_cases(cases)
        report_failures(cases.records)
        require(all(record["status"] == "PASS" for record in cases.records), "SP-003 pure codec check failed")


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_SP003_CHECK'))
