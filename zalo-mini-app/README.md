# Agri-Connect Zalo Mini App

The farmer's way in from Zalo: services near them (with a call button), crop prices, and a job post. It is a client of the backend's JSON API; it holds no data and no secret of its own.

| Screen   | API it calls                                                                   |
| -------- | ------------------------------------------------------------------------------ |
| Home     | `GET /api/explore/categories?type=service`, `GET /api/explore/prices`          |
| Near me  | `POST /api/zalo/location` (or an area: `GET /api/explore/locations`), `GET /api/explore/services?lat&lng&category_id`, `POST /api/explore/calls` |
| Prices   | `GET /api/explore/prices?location_id=`                                         |
| Post job | `POST /api/zalo/phone` (optional), `POST /api/explore/jobs`                    |

Zalo gives the position and the phone number as one-time tokens (`getLocation`, `getPhoneNumber`, valid 2 minutes); the backend converts them with the Zalo app's secret key (`ZALO_APP_SECRET`) through the Zalo Graph API. Without that secret both buttons answer `zalo_not_configured`, and the farmer chooses an area and types the number instead.

## Setup

1. Create the Mini App on [mini.zalo.me](https://mini.zalo.me) and link it to a Zalo app on [developers.zalo.me](https://developers.zalo.me); note the app's secret key.
2. Backend `.env`:
   - `ZALO_APP_SECRET=<the secret key>`
   - `CORS_ORIGINS=https://h5.zdn.vn` (the Mini App's origin; add it to any origins already there). A write refused as `cross_origin_request` means Zalo's webview sent another origin: the backend log names it; add that one too. In development, add the `npm run dev` origin (e.g. `http://localhost:5174`).
3. Here: copy `.env.example` to `.env.production` and set `VITE_API_URL` to the backend's public origin (its `APP_URL`).
4. In the Zalo Mini App console, request the location and phone number permissions; Zalo shows the farmer a prompt the first time each is used.

## Develop and deploy

```powershell
npm ci
npm run dev        # http://localhost:5173 is the backend's; vite picks the next free port
npm run build      # into www/
npm install -g zmp-cli   # once: the `zmp` command
zmp login                # once
npm run deploy     # zmp deploy: uploads a version for testing / review
```

Outside Zalo (a desktop browser) the screens render, but the position, phone and call buttons need the Zalo app.

`package.json` `overrides` lift two of zmp-ui's and zmp-vite-plugin's pins to releases with the security fixes (`npm audit` is clean with them): `react-router` / `react-router-dom` 7.18 (zmp-ui asks for 6; it uses only `BrowserRouter`, `MemoryRouter`, `Routes`, `Route`, `useNavigate`, `useLocation`, `useNavigationType`, `NavigationType`, all unchanged in 7) and vite 6.4 (zmp-vite-plugin asks for 4 or 5). When zmp-ui or zmp-vite-plugin allow these versions themselves, drop the override. Check a new Mini App version in Zalo's test view before submitting it.

## Conventions

- Every word the app shows is in `src/texts.js`; error messages come from the backend, already in Vietnamese.
- Large tap targets (44 px and up), little text: the users are farmers on low-end Android phones.
- No configuration in code: the API origin is `VITE_API_URL`.
