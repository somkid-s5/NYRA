# Preserve the endpoint across Web Diagnostics steps

Web Diagnostics will pass the selected TCP peer address and the original URL hostname to subsequent TLS and HTTP checks. Resolving independently in each step would allow DNS rotation to combine evidence from different servers. The hostname remains the identity used for TLS verification and HTTP routing.

The bundled workflow reports redirects and asks the operator to run a separate check for the new destination. Following redirects automatically would change the endpoint being diagnosed and the scope of network contact.
