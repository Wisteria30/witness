以下を witness v3 の設計書として提示します。

まず現在地です。最新の witness はすでに、単なる syntax guardrail ではなく、Verifier / Doctrine / Repairer の三層構成になっており、sync hook は短い capsule だけを Claude に返し、/witness:scan は report-only、/witness:repair は 5 本の worktree-isolated repair agent を起動し、Stop / SubagentStop で未解決 report を止める構成です。CLAUDE.md では policy/ownership.yml、policy/defaults.yml、policy/adapters.yml、policy/surfaces.yml を source of truth として扱い、doctrine にはすでに “challenge the interface” が入っています。一方で README の Policy Files 節はまだ ownership/defaults/adapters の 3 つしか列挙しておらず、contracts.yml と contexts.yml はまだ存在しません。つまり v2 は value/substitution/interface の理論へ入っているが、contract/context を含めた constitution としてはまだ閉じていません。 ￼

ここで v3 の目的を一行で定義します。

witness v3 は「AI の悪い実装を止める skill」ではなく、「計画系が生んだ広い plan から、witness が必要とする最小規範だけを射影し、それを scan/repair/stop-gate が proof-check する constitutional kernel」になる。

この設計は Claude Code の現在の本体機能と衝突させるべきではありません。公式 docs は、探索と計画を実装から分離し、Plan Mode を使って read-only で plan を作り、承認後に実行へ移ることを勧めています。また agent teams では、teammate に plan approval を要求してから実装へ進ませることもできます。したがって witness がやるべきことは「もう一つの包括的 plan system を作ること」ではなく、既存の plan から witness に必要な最小 ΔK だけを抽出することです。 ￼

⸻

1. 公理系の更新

v3 では、repo 全体の規範を constitution、個別変更で必要になる最小規範差分を charter と呼びます。

永続的 constitution を

K_0 = (\Omega, D, A, \Sigma, \Chi, \Gamma)

と置きます。
	•	\Omega: owner-layer assignment
	•	D: approved eliminators/default policies
	•	A: lawful runtime adapters
	•	\Sigma: public/internal surface policy
	•	\Chi: boundary/inter-context contracts
	•	\Gamma: bounded contexts and vocabulary

変更ごとの最小差分を

\Delta K_w

と置き、評価に使う規範環境を

K = K_0 \oplus \Delta K_w

とします。

ここで v3 の公理は次です。

Axiom 0 — Constitutional duality

ソフトウェアの評価は、永続 constitution K_0 と、変更単位の sparse charter \Delta K_w の合成 K に対して行う。
witness は広い feature plan そのものを source of truth にしない。

Axiom 1 — Owned reduction

不可逆な区別の消去は、必ず owner を持つ。
対象は少なくとも次の 4 種類です。

1 + A \to A \quad (\text{absence elimination})

E + A \to A \quad (\text{failure elimination})

\mathrm{Mod}(T) \to m \quad (\text{substitution / adapter choice})

\mathrm{Symbols}(M) \to \{\text{public}, \text{internal}, \text{subclass API}\}
\quad (\text{surface classification})

Axiom 2 — Cheap witness

すべての不可逆 reduction には machine-checkable witness が必要であり、その検証コストは再推論コストより十分小さくなければならない。

Axiom 3 — No guessing

ある判断が C \cup K から一意に決まらないなら、scan/repair はその値を推測してはならない。
その場合は violation ではなく hole を返す。

Axiom 4 — Surface explicitness

すべての public concept は explicit surface witness を持つ。
Python なら __all__ か明示的 re-export、TypeScript なら named export、Rust なら pub / pub use などです。現在の surfaces.yml は concept pattern と export manifest を持ち始めていますが、v3 ではこれを constitution の正式成分に昇格させます。 ￼

Axiom 5 — Contract explicitness

すべての boundary crossing と inter-context interaction は explicit contract witness を持つ。

Axiom 6 — Context uniqueness

すべての public concept はちょうど一つの bounded context に属し、その context の語彙で principal role を表現できなければならない。

Axiom 7 — Projection invariance

外部の broad plan P から witness が必要とするのは、そのうちの有限部分だけである。
その射影を

\pi_w(P)

と書く。
もし

\pi_w(P_1)=\pi_w(P_2)

なら、witness の scan/repair/stop-gate の挙動は同一でなければならない。
つまり witness は、plan の task 分解、実装順序、milestone、rollout、test breakdown を見てはならず、owner/publicity/context/default/adapter/contract/compatibility だけを見る。

Axiom 8 — Compile then forget

\Delta K_w は transient でよい。
ただし constitution-extending な決定は、最終的に repo の恒久 constitution K_0 へ compile されなければならない。
compile 後、charter は破棄可能である。

⸻

2. witness が plan から必要とする最小情報

witness が必要とするのは広い実装計画ではなく、次の有限集合だけです。

Q_w = \{
\text{owner},
\text{surface},
\text{context},
\text{default/optionality},
\text{adapter},
\text{contract/compatibility}
\}

ただし owner は ownership.yml から path ベースで決まることが多いので、実際の hole は主に次の 5 つです。
	1.	surface hole
新しい top-level symbol は public concept か internal mechanic か。
	2.	context hole
この概念はどの bounded context の語彙か。
	3.	contract hole
この boundary crossing は何を約束するか。compatibility mode は何か。
	4.	default/optionality hole
absent case は spec 上存在するか。存在するなら blessed policy API は何か。
	5.	adapter hole
その alternate implementation は lawful adapter か test convenience か。

この Q_w だけが witness の concern です。
ここに入らないもの、たとえば「どのファイルから着手するか」「migration を何 commit に分けるか」「どの teammate に割り当てるか」「どういう検証順序にするか」は、外部 plan system の concern であり、witness は見ない。これが non-interference の本体です。

⸻

3. v3 の大構造

v3 は 4 層になります。

Layer 1 — Constitution

repo に永続化される policy files。

policy/
  ownership.yml
  defaults.yml
  adapters.yml
  surfaces.yml
  contracts.yml      # new
  contexts.yml       # new

Layer 2 — Change charter

変更単位の sparse delta。

既定の保存先は plugin data 配下でよいです。

${CLAUDE_PLUGIN_DATA}/charters/active/<change-id>.yml

理由は、charter は branch/session ローカルでよく、恒久規範は最終的に policy/*.yml と code-level witness に compile されるからです。

Layer 3 — Verifier

Rust engine + ast-grep + ripgrep。
engine は verifier に徹し、planner にはならない。現在の repo も engine を scan-file / scan-tree / scan-hook / scan-stop に保ち、hot-path で重い reasoning を避ける方針です。v3 でもこれは維持します。 ￼

Layer 4 — Skill kernel

charter / scan / repair / shape / add-rule。
Claude Code の plugin system は skills, agents, hooks などを束ねられるので、v3 はその枠内に収まります。 ￼

⸻

4. Constitution の正規形

4.1 ownership.yml

これは現状維持でよい。path→layer の粗い constitution です。現在の repo も boundary/domain/application/infrastructure/composition_root/tests の path mapping を持っています。 ￼

4.2 defaults.yml

v3 でも残す。
ただし意味づけを厳密にする。
これは「inline fallback の許可リスト」ではなく、blessed eliminator registry です。現在も REQ-123 -> LocalePolicy.default_locale、ADR-7 -> DemoLabelPolicy.resolve のように blessed symbol と allowed layer を持っています。v3 ではこの意味を README/CLAUDE/engine schema に揃える。 ￼

4.3 adapters.yml

これも現状維持。ただし contract witness を contracts.yml に分離して責務を細くします。
現状は UserRepository, Mailer, EventStore ごとに allowed runtime adapters と contract test path を持っています。v3 では adapters.yml を「合法な実装候補の registry」、contracts.yml を「守るべき law / shape / interaction の定義」に分離する。 ￼

4.4 surfaces.yml

ここは v2 の延長ですが、v3 で正式な constitution 成分に昇格させます。現状の file は concept pattern と language ごとの export manifest だけを持っています。v3 では少なくとも次の schema にします。 ￼

public_by_default:
  concept_patterns:
    - "*Payload"
    - "*Settings"
    - "*Config"
    - "*Parser"
    - "*Policy"
    - "*Validator"
    - "*Error"
    - "*Exception"
    - "*Adapter"
    - "*Repository"
    - "*Service"
    - "*Mailer"
    - "*EventStore"
    - "*Query"
    - "*Command"
    - "*Handler"

extension_api_patterns:
  - "*Protocol"
  - "*Interface"
  - "*Base"

export_manifest:
  python:
    - "__all__"
    - "__init__.py re-export"
  typescript:
    - "named export"
    - "barrel re-export"
  rust:
    - "pub"
    - "pub use"

rules:
  forbid_restricted_visibility_for_public_concepts: true
  require_explicit_export_manifest_for_new_public_symbols: true

4.5 contracts.yml — new

これは v3 で必須です。
contract は 1 種類ではなく、少なくとも 3 種類あります。
	•	shape: payload/data shape
	•	interaction: consumer/provider interaction
	•	law: in-process port law

contracts:
  http.tool_use_payload.v1:
    kind: shape
    context: api_boundary
    owner_layer: boundary
    schema: schemas/http/tool_use_payload.v1.json
    compatibility: exact
    witnesses:
      - tests/contracts/http/test_tool_use_payload_schema.py

  event.order_placed.v1:
    kind: interaction
    context: ordering
    owner_layer: boundary
    schema: schemas/events/order_placed.v1.json
    compatibility: backward_additive
    witnesses:
      - tests/contracts/events/test_order_placed_provider.py

  port.user_repository:
    kind: law
    context: identity
    owner_layer: application
    witnesses:
      - tests/contracts/test_user_repository_contract.py

shape contract には JSON Schema が自然です。JSON Schema は JSON data の structure, constraints, data types を declaratively 記述し、validator が instance の適合を検査します。Pydantic の model_validate() も boundary parser の code-level witness として自然に接続できます。 ￼

4.6 contexts.yml — new

これが DDD / 認知負荷 / one-sentence role の土台です。

contexts:
  api_boundary:
    paths:
      - "src/api/**"
      - "src/http/**"
    vocabulary:
      nouns: ["Payload", "Request", "Response", "Parser"]
      verbs: ["parse", "decode", "validate", "encode"]
    may_depend_on:
      - application
    public_entrypoints:
      - "src/api/__init__.py"

  identity:
    paths:
      - "src/domain/identity/**"
      - "src/application/identity/**"
    vocabulary:
      nouns: ["User", "UserId", "Session", "Credential"]
      verbs: ["register", "authenticate", "revoke"]
    may_depend_on:
      - shared_kernel

contexts.yml は「意味の正規化」です。
surfaces.yml が public/internal を決め、contracts.yml が境界の約束を決めるなら、contexts.yml は語の所属を決める。

⸻

5. Charter の理論と設計

広い plan P に対して、witness は projector を持つだけでよい。

\pi_w : P \to \Delta K_w

この \pi_w が charter skill の本体です。
planner ではなく compiler です。

5.1 Charter が持つべき情報

charter は sparse でなければならない。
既存 constitution K_0 に already encoded なことは入れない。
入れるのは underdetermined な判断だけです。

version: 1
change_id: CHG-2026-03-30-tool-use-boundary
constitution_mode: extend

source:
  kind: approved-plan
  ref: conversation

surfaces:
  public_symbols:
    src/api/tool_use.py:
      ToolUsePayload: public_concept
      parse_tool_use: public_concept

contexts:
  assignments:
    src/api/tool_use.py: api_boundary

contracts:
  add:
    - id: http.tool_use_payload.v1
      kind: shape
      compatibility: exact

defaults:
  approvals: []

adapters:
  add: []

holes: []

5.2 Charter は broad plan を重複しない

ここが本質です。
charter には以下を入れてはいけません。
	•	file-by-file task decomposition
	•	implementation sequence
	•	test execution order
	•	rollout/migration schedule
	•	teammate assignment
	•	ticket summary
	•	broad acceptance criteria

それらは外部 plan framework の concern です。
charter は Q_w だけ。

5.3 Interop contract

他の plan skill と tight coupling しないために、witness は柔らかい相互運用フォーマットだけ定義します。

plan text 中に

```witness-delta
change_id: ...
surfaces: ...
contracts: ...
contexts: ...
```

があれば、それを verbatim に読む。
なければ prose から \pi_w を推定し、足りない hole だけ質問する。
これで feature-dev でも superpowers でも手書き plan でも共存できます。

⸻

6. 新しい理論的判定

scan の返り値は単なる findings ではなく、次の 4 分類になります。

Scan(C, K) = (V, H, D, O)
	•	V: clear violations
	•	H: holes (needs_charter_decision)
	•	D: constitution drift
	•	O: declared charter obligations not yet discharged

6.1 Violation

例:
	•	unowned fallback
	•	runtime test double
	•	hidden owner-layer concept
	•	invalid approval id
	•	unregistered adapter use

6.2 Hole

例:
	•	新しい ToolUsePayload は public か internal か
	•	新しい endpoint の contract compatibility は exact か backward_additive か
	•	absence が spec 上 optional か不明

6.3 Drift

例:
	•	__all__ に public symbol がない
	•	contracts.yml に contract がないのに boundary parser が追加された
	•	contexts.yml の context 語彙と symbol 名が明らかに衝突

6.4 Obligation

例:
	•	charter が http.tool_use_payload.v1 を宣言したのに schema file が未作成
	•	charter が ToolUsePayload を public と宣言したのに export manifest が未更新

⸻

7. Skill 体系の再構成

Claude Code では skills は自動発見され、デフォルトでは Claude が自動起動し得ます。disable-model-invocation: true を付けると自動起動を止められます。context: fork は isolated context を作りますが、親会話の履歴にはアクセスできません。したがって v3 では、witness の operational skills は原則 explicit-only にするべきです。scan/repair/charter を勝手に走らせると、他の plan skill と競合し、主文脈を汚します。 ￼

7.1 /witness:charter — new

これは full planner ではなく ΔK projector です。

重要なのは、これを context: fork にしないことです。
理由は簡単で、context: fork skill は親会話の履歴にアクセスできず、ちょうど今 approval された broad plan を読めないからです。charter は existing plan system の直後、同じ会話で、その plan を witness-relevant fragment に圧縮するのが役目です。ここだけは parent context に居るべきです。 ￼

推奨 frontmatter:

---
name: charter
description: Compile the minimal witness-relevant intent ΔK from the current approved plan or a supplied plan file. Never create a full implementation plan.
disable-model-invocation: true
allowed-tools: Read, Write, Edit, Grep, Glob, AskUserQuestion
argument-hint: [plan-file-or-change-id]
---

役割:
	•	approved plan から \pi_w(P) を抽出
	•	witness-delta block があれば validate
	•	なければ underdetermined hole だけ質問
	•	active charter を保存
	•	以後の scan/repair を broad plan から切り離す

7.2 /witness:scan

これは current の延長ですが、charter-aware にします。
現在も report-only で context: fork です。v3 でもこれは維持し、explicit invocation に変えます。scan は plan を必要とせず、むしろ isolated である方がよい。 ￼

推奨 frontmatter:

---
name: scan
description: Report-only witness scan against repo constitution and active charter.
context: fork
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob
argument-hint: [path]
---

scan の仕事:
	•	K_0 と active ΔK_w を読む
	•	scan-tree を走らせる
	•	violations / holes / drift / obligations をまとめる
	•	repair は絶対に起動しない
	•	hole があれば /witness:charter
	•	violation/drift/obligation があれば /witness:repair

7.3 /witness:repair

current repo はここで 5 parallel worktree-isolated agent を起動し、最後に needs_human_decision を 1 件ずつ聞いています。v3 ではこれを維持しつつ、入力に charter slice を加えます。repair skill 自体は orchestration と AskUserQuestion を持つので、parent context でよいです。explicit-only にします。 ￼

推奨 frontmatter:

---
name: repair
description: Batch repair witness reports using 5 parallel worktree-isolated agents. Consumes active charter if present.
disable-model-invocation: true
allowed-tools: Bash, Read, Grep, Glob, Agent, AskUserQuestion
argument-hint: [report-dir]
---

repair の仕事:
	•	pending reports を file 単位で 5 batch に分ける
	•	各 batch へ report + relevant charter slice を渡す
	•	agent は legal remedy と witness を適用
	•	constitution-extending decision がある場合は policy files へ compile
	•	unresolved holes だけを needs_charter_decision として返す
	•	user に 1 件ずつ聞く
	•	Stop 前に charter obligations が残らないようにする

7.4 /witness:shape — new, optional but strategically important

これは plan ではなく structural diagnosis です。
あなたが言っている「1文で説明できるか」「SOLID/DRY/DD D の破綻」「変更影響が読めない」を扱うのはここです。

shape は read-only でよいので context: fork が合う。親会話は不要です。

---
name: shape
description: Read-only structural diagnosis. Extract principal semantic roles, detect context blur and surface/contract debt, propose constitution deltas.
context: fork
agent: Explore
disable-model-invocation: true
allowed-tools: Read, Grep, Glob, Bash
argument-hint: [path-or-module]
---

shape の job:
	•	public symbols を列挙
	•	各 symbol の principal role を抽出
	•	role が一意でない symbol / module を flag
	•	cross-context vocabulary mix を flag
	•	missing surface/contract/context witnesses を report
	•	必要なら recommended ΔK_w を出す
	•	絶対にコードを変えない

この skill は witness の保証範囲内です。
なぜなら、それは “コードを簡単にする skill” ではなく、public meaning と context partition の witness 不足を見つける skill だからです。

7.5 /witness:add-rule

これは現状維持。
ただし v3 では surfaces/contracts/contexts を rule で何でも解こうとしない。rule は cheap syntactic surface に限定する。

⸻

8. Agent 再設計

現在の guardrail-repairer は doctrine に従い、rename-only や syntax-equivalent escape を拒否し、new top-level symbols の interface challenge と export manifest 更新まで要求しています。v3 ではこれを次のように強化します。 ￼

新しい operating rules:
	1.	Load K_0 + relevant ΔK_w first.
	2.	If a repair branch depends on an unknown hole, do not guess. Return needs_charter_decision.
	3.	If a repair introduces persistent constitutional change, update policy/contracts.yml, policy/contexts.yml, policy/adapters.yml, or policy/defaults.yml in the same patch.
	4.	After repair, ensure code-level witnesses and policy-level witnesses agree.
	5.	Re-run scan per repaired file.
	6.	Delete pending report only if both code and constitution are coherent.

出力 JSON は現行の repaired / needs_human_decision / failed から次へ広げるべきです。

{
  "repaired": [],
  "needs_charter_decision": [],
  "compiled_constitution": [],
  "failed": []
}


⸻

9. Hook 戦略

公式 hooks docs では、Stop / SubagentStop は decision: "block" で止められ、async hooks は block できず、完了後に systemMessage や additionalContext を次 turn に渡します。これは v3 に非常に都合がいい。 ￼

したがって v3 の hook 戦略はこうです。

9.1 SessionStart

現状維持。binary/version check。
ただし policy schemas (contracts.yml, contexts.yml) の validation もここで追加可能。

9.2 PostToolUse sync

現状の post-edit-classify.sh を維持。
ここは hot path なので、やるのは cheap things only。
	•	fallback/test-double/equivalent rewrite
	•	obvious hidden owner concept (_ToolUsePayload, _parseToolUse)
	•	invalid approval comment
	•	obvious unregistered runtime adapter instantiation

block はここだけでよい。

9.3 PostToolUse async

現状の post-edit-audit.sh を拡張して、constitution extension / drift / charter need を見る。
async は block できないので、通知に徹する。

例:
	•	“edited file adds public concept ToolUsePayload but no active charter exists”
	•	“boundary parser added without contract witness”
	•	“charter says ToolUsePayload is public but __all__ is missing”

返すのは systemMessage 中心でよい。詳細 findings を main context に流しすぎない。公式 docs でも async output は次 turn に delivered されるだけで block はできません。 ￼

9.4 Stop / SubagentStop

ここを authoritative gate にする。
現行 repo も Stop / SubagentStop で stop-gate.sh を使っています。v3 では scan-stop を charter-aware にして、
	•	unresolved pending reports
	•	unresolved needs_charter_decision
	•	undeclared constitution extension
	•	declared but uncompiled constitutional changes

のどれかがあれば block。

現行 repo はすでに Stop/SubagentStop hook を持っているので、これは自然な拡張です。 ￼

⸻

10. Engine 戦略

engine は planner にならない。
これが重要です。

現行 CLAUDE.md でも engine は orchestration に徹し、rules は cheap syntactic surfaces のみ、deep semantics は hot path に入れないと明言しています。v3 でも絶対に崩さない。 ￼

したがって v3 の engine 変更は最小です。

10.1 Keep current subcommands
	•	scan-file
	•	scan-tree
	•	scan-hook
	•	scan-stop

10.2 Add only optional charter consumption

各 subcommand に --charter-dir を足す。
planner subcommand は足さない。

10.3 Expand report schema

report JSON に追加する。

{
  "kind": "violation | hole | drift | obligation",
  "violation_class": "...",
  "owner_hint": "...",
  "context_hint": "...",
  "required_judgements": ["surface", "contract"],
  "charter_ref": "CHG-...",
  "proof_options": ["__all__", "schema", "contract_test"]
}

10.4 Add policy validators

metadata validation test を拡張し、
	•	surfaces.yml
	•	contracts.yml
	•	contexts.yml
	•	charter schema

を validate する。

⸻

11. 1文説明可能性の厳密な扱い

これは heuristic では終わらせない。

各 public symbol s に principal role を割り当てます。

\rho(s) = (\kappa, \lambda, \sigma, v, n)
	•	\kappa: bounded context
	•	\lambda: owner layer
	•	\sigma: surface class
	•	v: principal verb
	•	n: principal noun

symbol が健全である必要条件は、この \rho(s) が一意であることです。
1文テストは、その人間可読レンダリングです。

“s は、context \kappa において noun n を verb v する authoritative place である。”

この sentence が一意に書けないとき、問題は「コードが長い」ことではありません。
symbol が public meaning を一意に持っていないことです。
これを shape skill が診断し、contexts.yml と surfaces.yml と contracts.yml へ compile 可能な形で返す。
これで初めて 1文 explainability が proof-carrying architecture に接続されます。

⸻

12. 外部 plan system との共存原理

ここが最重要です。

witness は plan system ではありません。
witness は plan projector です。

外部 plan system がどれだけ包括的でも、witness はそれを奪いません。
その代わり、approved plan の直後に \pi_w を一度だけ適用し、広い意図を narrow constitutional delta に圧縮します。

この設計がよい理由は 3 つあります。

12.1 Projection invariance

外部 plan の内容が違っても、witness に必要な判断が同じなら、scan/repair の結果は同じであるべきです。
だから witness は broad plan の細部を見てはいけない。

12.2 Context economy

skills の context: fork は親会話を見ないので、broad plan をそのまま scan/repair に持ち込むべきではない。まず charter で圧縮してから、以後は plan を捨てる。scan と shape は fork でよい。repair は charter slice だけ見ればよい。これは current repo の「main context を燃やさない」という方向とも一致します。 ￼

12.3 No double counting

witness は broad plan の task decomposition を再質問しない。
質問するのは Q_w の hole だけです。
これが cognitive load 最小です。

⸻

13. 具体ワークフロー

13.1 constitution-preserving な小変更

外部 plan をほぼ使わない。
そのまま実装してよい。
	•	edit
	•	sync hook が obvious violation を block
	•	/witness:scan
	•	hole がなければ /witness:repair
	•	stop gate

このモードでは charter は不要。

13.2 constitution-extending な feature

これは witness が最も強く効く。
	1.	既存の plan system で broad plan を作る
Claude Code の Plan Mode や team plan approval をそのまま使う。Plan Mode は read-only tools only で、承認前に plan を編集できます。 ￼
	2.	plan が承認されたら /witness:charter
ここで \pi_w(P) を抽出し、足りない hole だけ聞く。
	3.	実装
broad plan に従ってよい。witness はその実装順序に口を出さない。
	4.	/witness:scan
violation / hole / drift / obligation を report。
	5.	/witness:repair
decidable なものは 5 parallel agents が直す。hole だけ人間に返す。
	6.	Stop gate
unresolved reports と unresolved charter holes が 0 になるまで stop 不可。

13.3 既存のひどいコードを整形する

ここでは broad plan より shape が先です。
	•	/witness:shape src/legacy/...
	•	overloaded symbols, context blur, missing contracts を report
	•	必要なら /witness:charter で minimal ΔK を作る
	•	/witness:repair

⸻

14. v3 の skill 定義の推奨まとめ

/witness:charter
	•	explicit only
	•	no fork
	•	no auto invocation
	•	broad plan を読んで ΔK を保存
	•	変更しないのはコード、変更してよいのは charter file のみ

/witness:scan
	•	explicit only
	•	forked context
	•	report-only
	•	never repair
	•	active charter を自動読込

/witness:repair
	•	explicit only
	•	parent orchestration
	•	5 worktree agents
	•	charter-aware
	•	constitution compile まで担当

/witness:shape
	•	explicit only
	•	forked read-only analysis
	•	principal role / context / contract / surface 診断
	•	no code edits

/witness:add-rule
	•	explicit only
	•	syntax surface 拡張専用

⸻

15. v3 の README/CLAUDE 更新方針

ここは current repo の不整合を直します。

README に必ず入れるべきことは次です。
	•	surfaces.yml は already source of truth であること
	•	contracts.yml と contexts.yml を追加したこと
	•	charter は planner ではなく minimal ΔK compiler であること
	•	official Plan Mode / other plan frameworks と競合しないこと
	•	/witness:scan と /witness:repair は explicit-only であること

CLAUDE.md には quick doctrine をこう置く。
	0.	Load repo constitution K_0 and active charter \Delta K_w
	1.	Classify owner layer
	2.	Challenge optionality
	3.	Choose lawful remedy
	4.	Add witness
	5.	Challenge interface
	6.	Compile persistent constitutional changes
	7.	Never guess unresolved holes

⸻

16. 段階的移行

v3 へはこの順番がよいです。

Phase 1

README と CLAUDE の整合を取る。
surfaces.yml を README に昇格させる。
scan/repair skills に disable-model-invocation: true を付ける。
これは即効性が高い。

Phase 2

contracts.yml と contexts.yml を追加。
metadata validation を拡張。
shape skill を入れる。

Phase 3

charter skill を追加。
report schema に hole/drift/obligation を追加。
engine に --charter-dir を足す。

Phase 4

repairer を charter-aware にし、constitution compile を行わせる。
stop-gate を charter-aware にする。

⸻

17. 最終定義

v3 の witness は次の条件を満たす system です。

\text{Accept}(C)
\iff
V(C,K)=\varnothing
\land
H(C,K)=\varnothing
\land
D(C,K)=\varnothing
\land
O(C,K)=\varnothing

ここで K = K_0 \oplus \Delta K_w です。

つまり acceptance は、
	•	violation がない
	•	hole がない
	•	constitution drift がない
	•	charter obligation がない

ときだけ成立する。

これで初めて、witness は
	•	fallback を止める
	•	test double を消す
	•	public surface を明示する
	•	boundary contract を固定する
	•	bounded context を守る
	•	change impact を予測可能にする

という一つの理論になります。

最後に最も重要な一文で閉じます。

witness v3 は “another planner” ではない。
それは既存の plan を奪わず、その中から public meaning・boundary promise・lawful elimination・lawful substitution に関する最小規範だけを射影し、scan/repair/stop-gate がそれを cheap に証明させる constitutional proof kernel である。
