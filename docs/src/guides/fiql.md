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
curl -u admin:pw 'localhost:8080/rest/v1/targets?q=updateStatus%3D%3Derror'
```

## Filterable fields

Each resource exposes its own field map; an unknown field returns `400 Bad
Request`. Common maps:

- **targets** — `id`/`controllerId`, `name`, `description`, `updateStatus`,
  `lastControllerRequestAt`, `address`
- **actions** — `id`, `active`, `detailStatus`
- **rollouts** — `id`, `name`, `status`
- **target filters** — `id`, `name`

Boolean fields (e.g. `active`) accept `true`/`false` and compile to typed boolean
comparisons.

## Where FIQL is used

Beyond `q=` on list endpoints, the same grammar drives:

- **Rollout** target selection (`targetFilterQuery`).
- **Target filter** queries and their auto-assignment matching.

The query is validated when a rollout or target filter is created, so an invalid
expression is rejected up front rather than failing silently later.
