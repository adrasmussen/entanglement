#!/bin/bash
# checks the dev certs from docker/certgen/generate.sh: the CA is name-constrained, the
# server cert meets the rules browsers apply even to a locally trusted CA, a cert the CA
# signs for any other site is rejected, and the running server verifies end to end.
# prints a grid with a computed match column; exits non-zero on any mismatch.
#
#   bash docker/devtest/cert_check.sh        (from the repo root, dev stack up)
#
# it signs one throwaway "rogue" cert in a temp dir to prove the constraints, and deletes it.
set -u
D=docker/dev-certs
URL=https://127.0.0.1:8444/entanglement
fails=0
printf '%-64s | %-14s | %-14s | %s\n' case expected observed match
printf -- '-%.0s' {1..110}; echo
row() {
  local m=OK; [ "$2" = "$3" ] || { m=FAIL; fails=$((fails + 1)); }
  printf '%-64s | %-14s | %-14s | %s\n' "$1" "$2" "$3" "$m"
}
has() { grep -q -- "$2" <<<"$1" && echo yes || echo no; }

ca=$(openssl x509 -in $D/ca.crt -noout -ext basicConstraints,nameConstraints 2>/dev/null)
row "CA: basicConstraints CA:TRUE, pathlen:0" yes "$(has "$ca" 'CA:TRUE, pathlen:0')"
row "CA: name constraints are critical" yes "$(has "$ca" 'Name Constraints: critical')"
for n in DNS:localhost DNS:server DNS:postgres IP:127.0.0.1/255.255.255.255; do
  row "CA: permits $n" yes "$(has "$ca" "$n")"
done

srv=$(openssl x509 -in $D/server.crt -noout -ext subjectAltName,basicConstraints,extendedKeyUsage 2>/dev/null)
start=$(openssl x509 -in $D/server.crt -noout -startdate | cut -d= -f2)
end=$(openssl x509 -in $D/server.crt -noout -enddate | cut -d= -f2)
days=$(python3 -I -c 'import sys,datetime as d; f=lambda s: d.datetime.strptime(s, "%b %d %H:%M:%S %Y GMT"); print((f(sys.argv[2]) - f(sys.argv[1])).days)' "$start" "$end")
row "server: validity <= 825 days (macOS rule)" yes "$([ "$days" -le 825 ] && echo yes || echo "no ($days)")"
row "server: EKU serverAuth (macOS rule)" yes "$(has "$srv" 'TLS Web Server Authentication')"
row "server: CA:FALSE" yes "$(has "$srv" 'CA:FALSE')"
row "server: SAN has 127.0.0.1 and localhost" yes "$( [ "$(has "$srv" 'IP Address:127.0.0.1')$(has "$srv" 'DNS:localhost')" = yesyes ] && echo yes || echo no)"
row "server: chains to the CA (openssl verify)" OK "$(openssl verify -CAfile $D/ca.crt $D/server.crt 2>&1 | awk '{print $NF}')"
row "server: not expiring within 30 days" yes "$(openssl x509 -in $D/server.crt -noout -checkend 2592000 >/dev/null && echo yes || echo no)"
row "client: chains to the CA for client auth" OK "$(openssl verify -CAfile $D/ca.crt -purpose sslclient $D/client.crt 2>&1 | awk '{print $NF}')"

# a cert for a real site signed by the dev CA must be rejected
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
openssl req -newkey rsa:2048 -nodes -keyout $T/r.key -out $T/r.csr -subj /CN=www.example.com >/dev/null 2>&1
printf 'subjectAltName=DNS:www.example.com\nextendedKeyUsage=serverAuth\nbasicConstraints=CA:FALSE\n' >$T/r.ext
openssl x509 -req -in $T/r.csr -CA $D/ca.crt -CAkey $D/ca.key -CAserial $T/r.srl -CAcreateserial \
  -out $T/r.crt -days 30 -extfile $T/r.ext >/dev/null 2>&1
row "rogue www.example.com cert from the dev CA: openssl" "subtree" "$(openssl verify -CAfile $D/ca.crt $T/r.crt 2>&1 | grep -q 'permitted subtree violation' && echo subtree || echo accepted)"
if command -v security >/dev/null; then
  row "rogue www.example.com cert from the dev CA: macOS" "constraints" "$(security verify-cert -c $T/r.crt -r $D/ca.crt -p ssl -s www.example.com -L -l -v 2>&1 | grep -q 'Name constraints violated' && echo constraints || echo other)"
fi

# live: verified TLS (no -k) against the running server
C=(--cacert $D/ca.crt --cert $D/client.crt --key $D/client.key)
get() { local o; for _ in 1 2 3; do o=$(curl -s -o /dev/null -w '%{http_code}' "$@"); [ "$o" != 000 ] && break; done; echo "$o"; }
row "live: app over 127.0.0.1, server verified, client cert" 200 "$(get "${C[@]}" $URL/app)"
row "live: API over localhost, server verified, client cert" 200 "$(get "${C[@]}" -X POST -H 'content-type: application/json' -d '{"filter":""}' https://localhost:8444/entanglement/api/SearchCollections)"
row "live: no client cert is refused" 000 "$(curl -s -o /dev/null -w '%{http_code}' --cacert $D/ca.crt $URL/app)"

echo
[ $fails -eq 0 ] && echo "ALL CASES MATCH" || echo "$fails CASE(S) FAILED"
exit $((fails > 0))
