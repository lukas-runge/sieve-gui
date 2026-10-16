# Dovecot 2.4 + Pigeonhole (ManageSieve) for the managesieve integration tests.
# The official image ships Pigeonhole and runs rootless: ManageSieve listens
# on 34190 (STARTTLS) inside the container; test.conf adds 35190 (implicit TLS).
FROM dovecot/dovecot:2.4.5

COPY certs/tls.crt certs/tls.key /etc/dovecot/test-certs/
COPY test.conf /etc/dovecot/conf.d/zz-test.conf

# Any username is accepted with this password (static passdb from the image).
ENV USER_PASSWORD=secret
