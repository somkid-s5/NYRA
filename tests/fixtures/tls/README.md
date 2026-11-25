# Local HTTPS test certificates

These files are test fixtures only. `localhost-cert.der` is a certificate for
`localhost`, signed by `localhost-ca.pem`; `localhost-key.der` is its public
test-server key. The Linux CLI integration test passes the CA bundle to the
NYRA child process with `SSL_CERT_FILE`. It does not add the CA to any machine
trust store. Never reuse the test key or CA outside loopback tests.

The root certificate is valid from 2026-11-18 through 2036-11-15 UTC. Refresh
the certificate fixtures before expiration and rerun the Linux trusted HTTPS
CLI test.
