# Versioning And Changelog Policy

## Versioning

- retained CLI surface를 깨면 minor가 아니라 breaking release로 취급한다.
- provider compatibility minimum을 올릴 때는 CHANGELOG와 decision record를 같이 남긴다.
- experimental capability 변화는 primary product contract와 분리해 기록한다.
- public exit code도 retained CLI surface 일부로 본다.

## Exit Codes

- `0`: success
- `2`: parse error
- `3`: config error
- `4`: release gate error
- `5`: runtime init error
- `6`: runtime execution error
- `7`: runtime shutdown error

## Changelog

- retained commands 변화는 첫 줄에 드러나야 한다.
- removed product surface와 experimental surface는 분리해서 적는다.
- substrate pin 변경은 version, 이유, rollback condition과 함께 적는다.
- exit code 변경은 breaking change로 적는다.
