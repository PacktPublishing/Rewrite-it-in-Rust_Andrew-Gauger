# cairn-auth: the test Entra tenant

`cairn-auth` signs Cairn's sales team in through Microsoft Entra ID. To run
it, you need an Entra tenant with an app registration and a few test users.
This folder sets them up from the command line with the Azure CLI, so you
never have to click through the portal.

## 1. Get a tenant

A personal Microsoft account (an Outlook.com or Gmail-based sign-in) doesn't
come with an Entra tenant. Signing in to the portal with one lands you in
Microsoft's own "Microsoft Services" tenant, where every token request fails
with `AADSTS16000`, and **Manage tenants** is empty with **Create** greyed
out.

The simplest way to get a tenant of your own is an Azure free account at
<https://azure.microsoft.com/free>. It asks for a card, but Entra ID Free and
app registrations cost nothing. Signing up creates a tenant named **Default
Directory** with a domain like `yournamegmail.onmicrosoft.com`, and makes you
its Global Administrator.

## 2. Install the Azure CLI and sign in

```sh
brew install azure-cli                     # macOS; see Microsoft's docs for Windows and Linux
az login --tenant yournamegmail.onmicrosoft.com --allow-no-subscriptions
```

Naming the tenant matters. Without `--tenant`, a personal account signs in
to "Microsoft Services" again.

If the Entra admin center keeps asking you to sign in again
(`AADSTS50058`), your browser is blocking third-party cookies. Allow them
for `[*.]microsoftonline.com`, `[*.]live.com`, `[*.]microsoft.com` and
`[*.]azure.com`, and open the admin center with the tenant in the address:
`https://entra.microsoft.com/?tenant=yournamegmail.onmicrosoft.com`.

## 3. Run the setup

```sh
sh setup-entra.sh yournamegmail.onmicrosoft.com
```

It's safe to run again. It reuses whatever already exists and creates:

| What | Details |
|---|---|
| `admin@<domain>` | A Global Administrator that belongs to the tenant. Use it instead of your personal account from now on |
| `pshah`, `mtorres`, `dliu` | Priya Shah, Maya Torres and Derek Liu, with V1's user names |
| `bhartley` | Ben Hartley, who left in 2021. V1 still lets him sign in; here his account is disabled |
| The `cairn-auth` app registration | Single tenant, Web redirect URI `http://localhost:3000/auth/callback` (override with `CAIRN_AUTH_REDIRECT`) |
| The `Cairn.Admin` app role | Assigned to Priya Shah. It arrives in her ID token's `roles` claim, and becomes `is_admin` |
| A client secret | Valid for one year. Created only when `.env` doesn't already hold one |

Nothing secret is printed. The script writes two files here, both readable
only by you and ignored by git:

- `.env`: the issuer, client ID, client secret and redirect URI that
  `cairn-auth` reads.
- `.env.users`: each new user's first password. Everyone must change it at
  first sign-in, and new tenants also require Microsoft Authenticator.

To start again, delete the `cairn-auth` app registration and the users in
the admin center, and delete `.env` and `.env.users`.
