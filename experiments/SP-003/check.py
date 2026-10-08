"""Pure SP-003 goldens and actual Rust domain-codec checks for shared verification."""
from build import build
from run import Cases, pure_cases, require


def main():
    binary, output = build()
    print(output, end="")
    cases = Cases(binary)
    pure_cases(cases)
    require(all(record["status"] == "PASS" for record in cases.records), "SP-003 pure codec check failed")


if __name__ == "__main__":
    main()
