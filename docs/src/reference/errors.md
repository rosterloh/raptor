# Error Codes

raptor returns hawkBit-shaped error bodies so that existing clients — which
branch on the HTTP **status code** and sometimes the `errorCode` string — behave
identically against raptor.

## Body shape

```json
{
  "exceptionClass": "org.eclipse.hawkbit.repository.exception.EntityNotFoundException",
  "errorCode": "hawkbit.server.error.repo.entitiyNotFound",
  "message": "target not found"
}
```

The `exceptionClass` and `errorCode` strings mirror hawkBit's for the covered
cases (including hawkBit's historical `entitiy` spelling), because some clients
match on them. Most clients branch on the status code, which is the hard
contract.

## Status codes

| Code | Meaning | Example |
|---|---|---|
| `400 Bad Request` | invalid FIQL or malformed body | `q=bogusField==1`, incomplete DS assignment |
| `401 Unauthorized` | bad or missing credentials | wrong target token, missing Basic auth |
| `404 Not Found` | unknown entity | target / module / action doesn't exist |
| `409 Conflict` | duplicate key | module name+version+type, duplicate filter name |
| `410 Gone` | feedback for a non-active action | device reports on a finished/canceled action |

## Notable `errorCode` strings

| errorCode | Paired status |
|---|---|
| `hawkbit.server.error.repo.entitiyNotFound` | 404 |
| `hawkbit.server.error.rest.body.notReadable` | 400 |
| `hawkbit.server.error.unauthorized` | 401 |
| `hawkbit.server.error.repo.entitiyAlreadyExists` | 409 |
| `hawkbit.server.error.repo.actionNotActive` | 410 |

## Auth responses

`401` from the Management zone includes a `WWW-Authenticate: Basic` header so
tools prompt for credentials. The web console's own session checks use a "quiet"
variant that omits the header, to avoid triggering the browser's native Basic-auth
dialog on the single-page app.
