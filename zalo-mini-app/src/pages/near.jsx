import React, { useCallback, useEffect, useState } from "react";
import { Box, Button, Icon, Page, Text, useLocation, useNavigate } from "zmp-ui";

import { api } from "../api";
import { Loading } from "../components/loading";
import { PlacePicker } from "../components/place-picker";
import { ServiceCard } from "../components/service-card";
import { T } from "../texts";
import { messageOf } from "../zalo";

/**
 * Services near the farmer: where (the phone's position or an area), which
 * service (all, or one category from the home tiles), then the nearest
 * services with a call button. None: the way to post a job.
 */
export default function NearPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const [categories, setCategories] = useState([]);
  const [categoryId, setCategoryId] = useState(location.state?.categoryId ?? "");
  const [place, setPlace] = useState(null);
  const [services, setServices] = useState(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.categories().then(setCategories).catch(() => setCategories([]));
  }, []);

  const load = useCallback(() => {
    if (!place) return;
    setServices(null);
    setError("");
    api
      .servicesNear({ lat: place.lat, lng: place.lng, categoryId })
      .then(setServices)
      .catch((e) => setError(messageOf(e, T.failed)));
  }, [place, categoryId]);

  useEffect(load, [load]);

  return (
    <Page className="page">
      <Text.Title>{T.nearTitle}</Text.Title>
      <Box className="card">
        <PlacePicker value={place} onChange={setPlace} />
        <label className="field-label" htmlFor="near-category">{T.services}</label>
        <select
          id="near-category"
          className="native-select"
          value={categoryId}
          onChange={(e) => setCategoryId(e.target.value ? Number(e.target.value) : "")}
        >
          <option value="">{T.allCategories}</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>{c.name}</option>
          ))}
        </select>
      </Box>

      {!place && <Text className="muted center">{T.needPlace}</Text>}
      {place && (error || services === null) && <Loading error={error} onRetry={load} />}
      {place && services?.length === 0 && (
        <Box className="card center">
          <Text>{T.nearEmpty}</Text>
          <Text size="small" className="muted">{T.nearEmptyHint}</Text>
          <Button prefixIcon={<Icon icon="zi-post" />} onClick={() => navigate("/job", { state: { categoryId } })}>
            {T.postJob}
          </Button>
        </Box>
      )}
      {services?.map((s) => <ServiceCard key={s.id} service={s} />)}
    </Page>
  );
}
