# 1Password Secret References Cheat Sheet

Secret references are `op://...` URIs that point at a value in 1Password without putting the plaintext secret in a file.

## Format

```bash
op://<vault>/<item>/[section/]<field>
```

Examples:

```bash
op://app-prod/db/password
op://app-dev/github/token
op://Private/aws/access key id
```

Names can usually be replaced with item/vault/field IDs when names are awkward or duplicate.

## Read One Secret

```bash
op read "op://app-prod/db/password"
```

Useful flags:

```bash
op read --no-newline "op://app-prod/db/password"
op read --out-file ./key.pem "op://app-prod/server/ssh/key.pem"
```

## Use With Environment Variables

Put references in `.env`:

```bash
DATABASE_URL=op://app-dev/db/url
API_TOKEN=op://app-dev/api/token
```

Run the command through 1Password:

```bash
op run --env-file=.env -- cargo run
op run --env-file=.env -- npm test
```

`op run` replaces matching secret references only inside the subprocess environment.

## Shell Expansion Gotcha

This prints the reference, not the secret, because the shell expands `$API_TOKEN` before `op run` can replace it:

```bash
API_TOKEN=op://app-dev/api/token op run -- echo "$API_TOKEN"
```

Use a subshell when the command itself expands the variable:

```bash
API_TOKEN=op://app-dev/api/token op run -- sh -c 'echo "$API_TOKEN"'
```

## Metadata And Special Values

Use `attribute` or `attr` query parameters:

```bash
op read "op://app-prod/db/password?attribute=type"
op read "op://app-prod/login/one-time password?attribute=otp"
```

Common field attributes: `value`, `type`, `title`, `id`, `purpose`, `otp`.

## Environment Switching

Use a variable in the reference when vaults/items share the same shape:

```bash
APP_ENV=app-dev
DATABASE_URL=op://$APP_ENV/db/url
```

Then:

```bash
APP_ENV=app-prod op run --env-file=.env -- cargo run
```

## Rules Of Thumb

- Keep `op://...` references in committed config; keep plaintext out of git.
- Quote references that contain spaces or query parameters.
- Prefer stable IDs if names are duplicated or likely to change.
- Use `op run` for app processes and `op read` for one-off shell commands.

Official docs: <https://www.1password.dev/cli/secret-references>
