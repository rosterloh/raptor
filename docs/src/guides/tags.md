# Tags

A **tag** is a free-form label you attach to targets or distribution sets:
`beta`, `eu-west`, `kiosk`, `qa`. Tags carry no behaviour of their own — they
exist so you can organise a fleet and then select it again with the `tag==` FIQL
term, in list queries, saved target filters and rollouts.

Target tags and distribution-set tags are separate namespaces: a `beta` target
tag and a `beta` DS tag are unrelated tags.

## Creating tags

The create body is an **array**, like the other Management API create endpoints:

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targettags \
  -H 'Content-Type: application/json' \
  -d '[{"name":"beta","description":"early access","colour":"#00ff00"}]'
```

`name` is required and unique (`409 Conflict` on a duplicate); `description` and
`colour` are optional. Distribution-set tags are identical, at
`/rest/v1/distributionsettags`.

Fetch, update and delete follow the usual REST shape. `PUT` only changes the
fields present in the body:

```bash
curl -u admin:pw           localhost:8080/rest/v1/targettags/1
curl -u admin:pw -X PUT    localhost:8080/rest/v1/targettags/1 \
  -H 'Content-Type: application/json' -d '{"colour":"#ff0000"}'
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targettags/1
```

**Deleting a tag deletes its assignments, not the tagged entities.** Removing
the `beta` tag leaves every target that carried it in place, simply untagged.

Tag lists support the usual paging, `sort=` and `q=` parameters over `id`,
`name`, `description` and `colour`:

```bash
curl -u admin:pw 'localhost:8080/rest/v1/targettags?q=name==be*&sort=name:ASC'
```

## Assigning tags

Assign one target by controller id, or several at once with a JSON array body:

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targettags/1/assigned/dev-1
curl -u admin:pw -X POST localhost:8080/rest/v1/targettags/1/assigned \
  -H 'Content-Type: application/json' -d '["dev-2","dev-3"]'
```

Assignment is idempotent — re-assigning an already-tagged target succeeds and
changes nothing, so bulk calls are safe to retry. An unknown controller id fails
the whole call with `404`.

Unassign with the same paths and `DELETE`, and list what a tag holds with
`GET .../assigned` (a paged list of full target objects, accepting `offset`,
`limit`, `sort=` and `q=`):

```bash
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targettags/1/assigned/dev-1
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targettags/1/assigned \
  -H 'Content-Type: application/json' -d '["dev-2","dev-3"]'
curl -u admin:pw 'localhost:8080/rest/v1/targettags/1/assigned?limit=100'
```

Distribution-set tags work the same way, keyed by distribution set **id**:

```bash
curl -u admin:pw -X POST   localhost:8080/rest/v1/distributionsettags/1/assigned/7
curl -u admin:pw -X POST   localhost:8080/rest/v1/distributionsettags/1/assigned \
  -H 'Content-Type: application/json' -d '[8,9]'
curl -u admin:pw           localhost:8080/rest/v1/distributionsettags/1/assigned
```

## Filtering by tag

`tag` is available as a FIQL field on `/rest/v1/targets` and
`/rest/v1/distributionsets`:

```bash
curl -u admin:pw 'localhost:8080/rest/v1/targets?q=tag==beta'
curl -u admin:pw 'localhost:8080/rest/v1/targets?q=tag!=beta'
curl -u admin:pw 'localhost:8080/rest/v1/targets?q=tag=in=(beta,canary)'
curl -u admin:pw 'localhost:8080/rest/v1/distributionsets?q=tag==qa'
```

It accepts `==`, `!=`, `=in=` and `=out=` (plus `*` wildcards on the tag name);
the ordering operators return `400`. Negation applies to the membership rather
than the name, so `tag!=beta` means "not tagged beta" — a target that also
carries `stable` is still excluded.

Because saved target filters compile the same field map, `tag==` works there
too, which makes it a natural way to scope a rollout:

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targetfilters \
  -H 'Content-Type: application/json' \
  -d '{"name":"beta-ring","query":"tag==beta"}'
```

Combine it with any other term to narrow further:

```
tag==beta ; updateStatus==error
tag==eu-* , tag==us-*
```
