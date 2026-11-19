// What only Zalo can do, through zmp-sdk. The position and the phone come
// as one-time tokens that the backend converts with the app's secret
// (POST /api/zalo/location, /api/zalo/phone); the secret never reaches the
// phone.

import { getAccessToken, getLocation, getPhoneNumber, openPhone } from "zmp-sdk";

import { api, ApiError } from "./api";

/** `{ lat, lng }` of the phone, after Zalo asked the farmer. */
export async function currentPosition() {
  const { token } = await getLocation();
  const accessToken = await getAccessToken();
  return api.zaloLocation(accessToken, token);
}

/** The farmer's Zalo phone as "0912345678", after Zalo asked them. */
export async function zaloPhone() {
  const { token } = await getPhoneNumber();
  const accessToken = await getAccessToken();
  const { phone } = await api.zaloPhone(accessToken, token);
  return phone;
}

/** Counts the tap (never blocking the call), then dials. */
export async function callProvider(phone, serviceId) {
  api.recordCall(serviceId).catch(() => {});
  await openPhone({ phoneNumber: phone });
}

/** A failure as one line to show: the backend's own message (already in
 * Vietnamese), else `fallback` (Zalo's SDK errors are not worded for farmers). */
export function messageOf(error, fallback) {
  return error instanceof ApiError && error.message ? error.message : fallback;
}
