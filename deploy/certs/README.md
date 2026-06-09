# TLS certificates (Cloudflare Origin) — place on the VM, do NOT commit

The production nginx (`docker-compose.prod.yml` → `deploy/nginx.prod.conf`) serves
HTTPS to Cloudflare using a **Cloudflare Origin Certificate**. The cert + key are
git-ignored and must be created on the VM:

1. Cloudflare dashboard → **SSL/TLS → Origin Server → Create Certificate**
   (this is the *Origin* certificate, NOT a Client certificate).
   - Key type: RSA (default) is fine.
   - Hostnames: `rubc.app`, `*.rubc.app`
   - Validity: 15 years (default).
2. Save the two blocks Cloudflare shows you, on the VM, into this directory:
   - **Origin Certificate** → `deploy/certs/rubc.app.pem`
   - **Private Key**       → `deploy/certs/rubc.app.key`
   ```bash
   chmod 600 deploy/certs/rubc.app.key
   ```
3. Cloudflare **SSL/TLS → Overview → mode: Full (strict)**, and
   **Edge Certificates → Always Use HTTPS: On**.

These filenames are referenced by `deploy/nginx.prod.conf`. If you use different
names, update `ssl_certificate` / `ssl_certificate_key` there.
