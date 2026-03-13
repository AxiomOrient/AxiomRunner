# 01. Review Report

## 결론

현재 AxonRunner는 **방향이 맞고, 기반도 충분히 단단하다.**
특히 `core`는 작고 명확한 도메인 엔진으로 잘 정리되어 있고,
`apps`는 실제 제품면을 좁히는 데 성공했으며,
`adapters`는 `codek` 실행 기반과 essential tool surface를 갖추기 시작했다.

다만 제품 완성도 관점에서 아직 남아 있는 문제는 명확하다.

1. **truth surface drift**  
   README / CHANGELOG / legacy blueprint docs / 실제 CLI가 완전히 같은 제품을 말하지 않는다.

2. **state / trace naming drift**  
   같은 `ReadOnly` 상태가 저장 스냅샷에서는 `readonly`,
   표시/trace에서는 `read_only`로 표현된다.

3. **evidence quality gap**  
   patch evidence는 존재하지만 digest/metadata 중심이다.
   operator가 “무엇이 어떻게 바뀌었는가”를 직관적으로 보는 데는 아직 약하다.

4. **substrate lifecycle gap**  
   `codek`는 좋은 실행 기반이지만,
   AxonRunner 제품 수준의 version pin / compatibility matrix / doctor enforcement가 더 필요하다.

5. **verification surface gap**  
   계획 문서는 adapter tool 테스트를 전제로 서술하지만,
   실제 visible tests tree와 완전히 맞아떨어지지 않는다.
   테스트가 없는 것인지, 문서가 낡은 것인지, 트리 반영이 누락된 것인지 정리해야 한다.

## 총평

### 강점
- 구조가 짧다.
- 도메인 정책이 명확하다.
- 실패를 숨기지 않는 방향으로 많이 개선됐다.
- trace / replay / doctor가 제품의 본질에 들어왔다.
- tool 표면이 넓지 않으면서 자동화 핵심에 가깝다.

### 약점
- 사용자 진실 표면이 문서 곳곳에서 아직 어긋난다.
- 설명 가능성과 재현 가능성은 좋아졌지만, “변경 근거”의 해상도는 더 올려야 한다.
- `codek` 통합은 맞는 방향이지만 운영 계약이 충분히 공식화되지 않았다.

## 권고

다음 루프는 기능 확장이 아니라 **수렴과 잠금**이어야 한다.

- README / help / changelog / charter / doctor를 하나의 제품면으로 잠근다.
- mode schema를 하나로 통일한다.
- patch evidence를 operator-grade로 강화한다.
- `codek` 호환성 계약을 제품 계약으로 끌어올린다.
- test tree와 계획 문서를 1:1로 맞춘다.

## 판단

현재 상태는 **“설계 방향은 합격, 제품 완결성은 아직 한 루프 더 필요”** 이다.
이 저장소는 다시 넓히기보다,
지금 만든 좁은 표면을 완전히 닫는 쪽이 맞다.
