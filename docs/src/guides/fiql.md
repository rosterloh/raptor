# Filtering with FIQL

Every Management API list endpoint accepts a `q=` query in **FIQL** (Feed Item
Query Language, the same dialect hawkBit uses, also called RSQL). raptor compiles
it to a database query.

## Grammar

**Comparison operators:**

| Operator | Meaning |
|---|---|
| `==` | equal (supports `*` wildcards → SQL `LIKE`) |
| `!=` | not equal (supports `*` wildcards) |
| `=lt=` | less than |
| `=le=` | less than or equal |
| `=gt=` | greater than |
| `=ge=` | greater than or equal |
| `=in=` | in a list: `field=in=(a,b,c)` |
| `=out=` | not in a list |

**Logical operators:**

- `;` — AND
- `,` — OR
- AND binds tighter than OR; use parentheses to group.

**Wildcards:** `*` in a value becomes a SQL `LIKE` wildcard, so
`controllerId==dev-*` matches everything starting with `dev-`.

## Examples

```
updateStatus==error
controllerId==beta-*
updateStatus=in=(pending,error)
name==prod-* ; updateStatus==in_sync
updateStatus==error , updateStatus==pending
```

URL-encode the query when passing it on the command line:

```bash
curl -u admin:pw 'localhost:8088/rest/v1/targets?q=updateStatus%3D%3Derror'
```

## Filterable fields

Each resource exposes its own field map; an unknown field returns `400 Bad
Request`. Common maps:

- **targets** — `id`/`controllerId`, `name`, `description`, `updateStatus`,
  `lastControllerRequestAt`, `address`, `tag`, `attribute.<key>`
- **distribution sets** — `id`, `name`, `version`, `description`, `complete`,
  `tag`
- **target tags / DS tags** — `id`, `name`, `description`, `colour`
- **actions** — `id`, `active`, `detailStatus`
- **rollouts** — `id`, `name`, `status`
- **target filters** — `id`, `name`

Boolean fields (e.g. `active`) accept `true`/`false` and compile to typed boolean
comparisons.

## The `tag` field

`tag` is not a column but a membership test against the entity's tags, so it
only accepts `==`, `!=`, `=in=` and `=out=` — the ordering operators return
`400`. Negation applies to the membership, not the tag name: `tag!=beta` means
"not tagged beta", so a target tagged both `beta` and `stable` is excluded.

```
tag==beta                        # targets tagged beta
tag!=beta                        # targets not tagged beta
tag=in=(beta,canary)             # tagged with either
tag==beta ; updateStatus==error  # tagged beta and in error
```

The same term works on `/rest/v1/distributionsets` and inside saved target
filter queries (and therefore rollout target filters).

## The `attribute.<key>` field

Targets report free-form key/value attributes (`configData` — `zephyr`,
`hw_revision`, `kernel`, `rauc_slot`, ...). Prefix a key with `attribute.` to
filter on it, the same way `tag==` reaches the tag join table:

```
attribute.hw_revision==rev-C                          # exact match
attribute.hw_revision==rev-*                           # wildcard
attribute.kernel==6.6 ; updateStatus==error             # combined with a column
```

An unknown key matches no targets rather than returning `400` — attributes are
free-form and vary by device class, so there is nothing to validate against.
This works on the target list, saved target filters and rollouts, since all
three share the same query compiler.

## Where FIQL is used

Beyond `q=` on list endpoints, the same grammar drives:

- **Rollout** target selection (`targetFilterQuery`).
- **Target filter** queries and their auto-assignment matching.

The query is validated when a rollout or target filter is created, so an invalid
expression is rejected up front rather than failing silently later.
