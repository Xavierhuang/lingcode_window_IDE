---
title: Subprocessor List
slug: subprocessors
---

LingCode (operated by Aurelian Labs Inc.) uses select third-party subprocessors to deliver core product functionality. Each subprocessor processes customer personal data only as necessary to provide its service, and all are subject to appropriate data protection agreements where applicable.

### How LingCode Uses Subprocessors

To provide fast, reliable, and secure functionality, LingCode relies on a small number of carefully vetted third-party subprocessors. These vendors help us deliver essential capabilities such as hosting, billing, authentication, analytics, email, and hosted AI features.

Each subprocessor only processes customer personal data as needed to provide its service.

LingCode maintains contracts and data protection agreements with subprocessors where applicable. We do not sell customer data, and we do not share customer personal data with vendors for advertising or marketing purposes.

### AI Subprocessors

LingCode offers two modes for AI:

1. **Bring your own API key** — data goes directly from the customer to the model provider; LingCode does not process or store it.
2. **LingCode-hosted models** — LingCode sends customer prompts to its hosted inference provider to generate responses. These vendors act as subprocessors only for customers who choose this mode.

### Ongoing Updates

**Last Updated**: June 29, 2026

This subprocessor list is reviewed regularly. LingCode will notify customers of material changes in accordance with our [Terms](https://lingcode.dev/terms) and [Privacy Policy](https://lingcode.dev/privacy-policy).

---

## Infrastructure & Hosting

| Subprocessor       | Purpose                                                                      | Data Location      |
| ------------------ | --------------------------------------------------------------------------- | ------------------ |
| **DigitalOcean**   | Application hosting, managed database, and S3-compatible object storage (Spaces) | United States      |
| **Cloudflare**     | DNS, CDN, and edge compute (Workers)                                         | Global             |
| **Vercel**         | Hosting for customer-published applications                                  | United States / Global |
| **Netlify**        | Hosting for customer-published applications                                  | United States / Global |

---

## Billing & Payments

| Subprocessor | Purpose                                          | Data Location |
| ------------ | ------------------------------------------------ | ------------- |
| **Stripe**   | Payment processing and subscription management   | United States |

---

## Authentication

| Subprocessor | Purpose                                  | Data Location |
| ------------ | ---------------------------------------- | ------------- |
| **Google**   | OAuth sign-in                            | United States |
| **GitHub**   | OAuth sign-in and code/prototype export  | United States |
| **Apple**    | OAuth sign-in                            | United States |

---

## Analytics

| Subprocessor         | Purpose                                  | Data Location          |
| -------------------- | ---------------------------------------- | ---------------------- |
| **Google Analytics** | Website usage analytics (lingcode.dev)   | United States / Global |

---

## Email & Communication

| Subprocessor | Purpose                              | Data Location |
| ------------ | ------------------------------------ | ------------- |
| **Resend**   | Transactional email                  | United States |
| **Slack**    | Support and product notifications    | United States |

---

## AI Services (LingCode-Hosted Models)

_These subprocessors apply only when customers opt to use LingCode-hosted AI models. When customers supply their own API keys, data is sent directly to the provider and does not pass through LingCode's infrastructure._

| Subprocessor                  | Purpose                                                                  | Data Location |
| ----------------------------- | ------------------------------------------------------------------------ | ------------- |
| **LingCode-hosted inference** | Hosted chat model inference                                              | —             |
| **OpenAI**                    | Image generation and text embeddings (site search and vector features)  | United States |

---

## Services You Connect (Not LingCode Subprocessors)

Some features let you connect your own third-party accounts or supply your own API keys. In those cases, data flows under your agreement with that provider, and the provider is not a LingCode subprocessor. Examples include your own Supabase project, your own Firebase project, and any API keys you add for features such as SMS or text-to-speech in applications you build with LingCode.
