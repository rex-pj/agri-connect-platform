import React, { useEffect, useState } from "react";
import { Box, Button, Icon, Input, Page, Text, useLocation, useSnackbar } from "zmp-ui";

import { api } from "../api";
import { PlacePicker } from "../components/place-picker";
import { T } from "../texts";
import { messageOf, zaloPhone } from "../zalo";

const EMPTY = { description: "", requester_name: "", requester_phone: "" };

/**
 * A job post: the service, where, what, a name and a phone (typed, or the
 * Zalo number after Zalo asked). The backend checks every box and answers
 * one message per box; providers nearby that offer the service are told.
 */
export default function JobPage() {
  const location = useLocation();
  const { openSnackbar } = useSnackbar();
  const [categories, setCategories] = useState([]);
  const [categoryId, setCategoryId] = useState(location.state?.categoryId ?? "");
  const [place, setPlace] = useState(null);
  const [form, setForm] = useState(EMPTY);
  const [errors, setErrors] = useState({});
  const [sending, setSending] = useState(false);
  const [phoneBusy, setPhoneBusy] = useState(false);
  const [done, setDone] = useState(false);

  useEffect(() => {
    api.categories().then(setCategories).catch(() => setCategories([]));
  }, []);

  const set = (field) => (e) => setForm({ ...form, [field]: e.target.value });

  const fillPhone = async () => {
    setPhoneBusy(true);
    try {
      const phone = await zaloPhone();
      setForm((f) => ({ ...f, requester_phone: phone }));
    } catch (e) {
      openSnackbar({ text: messageOf(e, T.failed), type: "error" });
    } finally {
      setPhoneBusy(false);
    }
  };

  const submit = async () => {
    setSending(true);
    setErrors({});
    try {
      await api.postJob({
        category_id: categoryId || null,
        location_id: place?.locationId ?? null,
        lat: place?.fromPhone ? place.lat : null,
        lng: place?.fromPhone ? place.lng : null,
        ...form,
      });
      setDone(true);
      setForm(EMPTY);
    } catch (e) {
      setErrors(e.fields || {});
      openSnackbar({ text: messageOf(e, T.failed), type: "error" });
    } finally {
      setSending(false);
    }
  };

  if (done) {
    return (
      <Page className="page">
        <Box className="card center">
          <Icon icon="zi-check-circle-solid" size={48} className="up" />
          <Text.Title>{T.jobPosted}</Text.Title>
        </Box>
      </Page>
    );
  }

  return (
    <Page className="page">
      <Text.Title>{T.jobTitle}</Text.Title>
      <Text size="small" className="muted">{T.jobSub}</Text>
      <Box className="card form">
        <label className="field-label" htmlFor="job-category">{T.jobCategory}</label>
        <select
          id="job-category"
          className={`native-select${errors.category_id ? " native-select--error" : ""}`}
          value={categoryId}
          onChange={(e) => setCategoryId(e.target.value ? Number(e.target.value) : "")}
        >
          <option value="">{T.chooseCategory}</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>{c.name}</option>
          ))}
        </select>
        {errors.category_id && <Text size="small" className="field-error">{errors.category_id}</Text>}

        <Text className="field-label">{T.jobWhere}</Text>
        <PlacePicker value={place} onChange={setPlace} error={errors.location_id} />

        <Input.TextArea
          label={T.jobDescription}
          placeholder={T.jobDescriptionPlaceholder}
          value={form.description}
          onChange={set("description")}
          status={errors.description ? "error" : undefined}
          errorText={errors.description}
        />
        <Input
          label={T.name}
          placeholder={T.namePlaceholder}
          value={form.requester_name}
          onChange={set("requester_name")}
          status={errors.requester_name ? "error" : undefined}
          errorText={errors.requester_name}
        />
        <Input
          label={T.phone}
          type="text"
          inputMode="tel"
          placeholder="0901234567"
          value={form.requester_phone}
          onChange={set("requester_phone")}
          status={errors.requester_phone ? "error" : undefined}
          errorText={errors.requester_phone}
          helperText={T.phoneHint}
        />
        <Button variant="tertiary" loading={phoneBusy} prefixIcon={<Icon icon="zi-call" />} onClick={fillPhone}>
          {T.useZaloPhone}
        </Button>
        <Button size="large" fullWidth loading={sending} prefixIcon={<Icon icon="zi-post" />} onClick={submit}>
          {T.submitJob}
        </Button>
      </Box>
    </Page>
  );
}
