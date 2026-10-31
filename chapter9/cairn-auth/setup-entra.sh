#!/bin/sh
# Sets up a test Microsoft Entra tenant for cairn-auth, with the Azure CLI.
#
#   az login --tenant <domain>.onmicrosoft.com --allow-no-subscriptions
#   sh setup-entra.sh <domain>.onmicrosoft.com
#
# Creates, or reuses when they already exist:
#   - a native Global Administrator, admin@<domain>
#   - Cairn's test reps, named as in V1's Users table; bhartley is disabled,
#     because Ben Hartley left in 2021
#   - the cairn-auth app registration: single tenant, a Web redirect URI,
#     and the Cairn.Admin app role, assigned to Priya Shah
#   - a client secret, only when .env doesn't already hold one
#
# Secrets never go to the terminal. The client secret goes to .env, the
# users' first passwords to .env.users, both mode 600 and ignored by git.
# Each user must change their password at first sign-in.
set -eu

domain=${1:?usage: sh setup-entra.sh <domain>.onmicrosoft.com}
redirect=${CAIRN_AUTH_REDIRECT:-http://localhost:3000/auth/callback}
here=$(cd "$(dirname "$0")" && pwd)
env_file="$here/.env"
users_file="$here/.env.users"
graph=https://graph.microsoft.com/v1.0
global_admin=62e90394-69f5-4237-9190-012177145e10 # built-in role template

tenant=$(az account show --query tenantId -o tsv)
signed_in_domain=$(az rest --method GET --url "$graph/domains" \
    --query "value[?isInitial].id | [0]" -o tsv)
if [ "$signed_in_domain" != "$domain" ]; then
    echo "az is signed in to $signed_in_domain, not $domain." >&2
    echo "Run: az login --tenant $domain --allow-no-subscriptions" >&2
    exit 1
fi
echo "Tenant $domain ($tenant)"

umask 077
touch "$users_file"

# user <name> <first> <last> <enabled>: prints the user's object ID.
user() {
    upn="$1@$domain"
    id=$(az ad user list --filter "userPrincipalName eq '$upn'" \
        --query "[0].id" -o tsv)
    if [ -z "$id" ]; then
        pw="Cairn-$(openssl rand -hex 8)"
        id=$(az ad user create --display-name "$2 $3" \
            --user-principal-name "$upn" --password "$pw" \
            --force-change-password-next-sign-in true \
            --query id -o tsv)
        az rest --method PATCH --url "$graph/users/$id" \
            --body "{\"givenName\":\"$2\",\"surname\":\"$3\"}" >/dev/null
        echo "$upn $pw" >>"$users_file"
        echo "  created $upn" >&2
    else
        echo "  exists  $upn" >&2
    fi
    az rest --method PATCH --url "$graph/users/$id" \
        --body "{\"accountEnabled\":$4}" >/dev/null
    echo "$id"
}

echo "Users"
admin_id=$(user admin Cairn Admin true)
pshah_id=$(user pshah Priya Shah true)
user mtorres Maya Torres true >/dev/null
user dliu Derek Liu true >/dev/null
user bhartley Ben Hartley false >/dev/null

has_role=$(az rest --method GET --url "$graph/roleManagement/directory/roleAssignments?\$filter=principalId%20eq%20'$admin_id'%20and%20roleDefinitionId%20eq%20'$global_admin'" \
    --query "length(value)" -o tsv)
if [ "$has_role" = 0 ]; then
    az rest --method POST --url "$graph/roleManagement/directory/roleAssignments" \
        --body "{\"principalId\":\"$admin_id\",\"roleDefinitionId\":\"$global_admin\",\"directoryScopeId\":\"/\"}" >/dev/null
    echo "  admin@$domain is Global Administrator"
fi

echo "App registration"
app_id=$(az ad app list --display-name cairn-auth --query "[0].appId" -o tsv)
if [ -z "$app_id" ]; then
    role_id=$(uuidgen | tr 'A-Z' 'a-z')
    roles=$(mktemp)
    trap 'rm -f "$roles"' EXIT
    cat >"$roles" <<EOF
[{"id": "$role_id", "allowedMemberTypes": ["User"], "isEnabled": true,
  "displayName": "Admin", "value": "Cairn.Admin",
  "description": "Can manage Cairn's reference data and sign-ins"}]
EOF
    app_id=$(az ad app create --display-name cairn-auth \
        --sign-in-audience AzureADMyOrg --web-redirect-uris "$redirect" \
        --app-roles "@$roles" --query appId -o tsv)
    echo "  created cairn-auth ($app_id)"
else
    echo "  exists  cairn-auth ($app_id)"
fi
role_id=$(az ad app show --id "$app_id" \
    --query "appRoles[?value=='Cairn.Admin'].id | [0]" -o tsv)

sp_id=$(az ad sp list --filter "appId eq '$app_id'" --query "[0].id" -o tsv)
[ -n "$sp_id" ] || sp_id=$(az ad sp create --id "$app_id" --query id -o tsv)

assigned=$(az rest --method GET --url "$graph/servicePrincipals/$sp_id/appRoleAssignedTo" \
    --query "length(value[?principalId=='$pshah_id' && appRoleId=='$role_id'])" -o tsv)
if [ "$assigned" = 0 ]; then
    az rest --method POST --url "$graph/servicePrincipals/$sp_id/appRoleAssignedTo" \
        --body "{\"principalId\":\"$pshah_id\",\"resourceId\":\"$sp_id\",\"appRoleId\":\"$role_id\"}" >/dev/null
    echo "  pshah@$domain has Cairn.Admin"
fi

if grep -q '^CAIRN_AUTH_CLIENT_SECRET=.' "$env_file" 2>/dev/null; then
    echo "  .env already holds a client secret; left as it is"
else
    secret=$(az ad app credential reset --id "$app_id" --append \
        --display-name cairn-auth-dev --years 1 --query password -o tsv \
        2>/dev/null)
    cat >"$env_file" <<EOF
CAIRN_AUTH_ISSUER=https://login.microsoftonline.com/$tenant/v2.0
CAIRN_AUTH_CLIENT_ID=$app_id
CAIRN_AUTH_CLIENT_SECRET=$secret
CAIRN_AUTH_REDIRECT_URI=$redirect
EOF
    echo "  wrote .env with a new client secret (expires in a year)"
fi

echo "Done. First passwords are in .env.users."
