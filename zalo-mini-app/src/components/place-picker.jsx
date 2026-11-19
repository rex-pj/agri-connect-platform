import React, { useEffect, useState } from "react";
import { Button, Icon, Text } from "zmp-ui";

import { api } from "../api";
import { T } from "../texts";
import { currentPosition, messageOf } from "../zalo";

/**
 * Where the farmer is: "use my position" (Zalo asks, the backend reads the
 * token) or an area preset. `onChange({ lat, lng, locationId })`: the
 * phone's position (no area), or the chosen area's centre with its id.
 * `error`: a message to show under the area box.
 */
export function PlacePicker({ value, onChange, error }) {
  const [areas, setAreas] = useState([]);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");

  useEffect(() => {
    api.areas().then(setAreas).catch(() => setAreas([]));
  }, []);

  const usePosition = async () => {
    setBusy(true);
    setProblem("");
    try {
      const { lat, lng } = await currentPosition();
      onChange({ lat, lng, locationId: null, fromPhone: true });
    } catch (e) {
      setProblem(messageOf(e, T.needPlace));
    } finally {
      setBusy(false);
    }
  };

  const chooseArea = (event) => {
    const id = Number(event.target.value);
    const area = areas.find((a) => a.id === id);
    onChange(area ? { lat: area.lat, lng: area.lng, locationId: area.id, fromPhone: false } : null);
  };

  return (
    <div className="place-picker">
      <Button
        variant="secondary"
        fullWidth
        loading={busy}
        prefixIcon={<Icon icon="zi-location" />}
        onClick={usePosition}
      >
        {busy ? T.positionBusy : T.usePosition}
      </Button>
      {value?.fromPhone && <Text size="small" className="place-picker__done">{T.positionSet}</Text>}
      {problem && <Text size="small" className="field-error">{problem}</Text>}
      <label className="field-label" htmlFor="place-area">{T.orArea}</label>
      <select
        id="place-area"
        className={`native-select${error ? " native-select--error" : ""}`}
        value={value?.locationId ?? ""}
        onChange={chooseArea}
      >
        <option value="">{T.chooseArea}</option>
        {areas.map((a) => (
          <option key={a.id} value={a.id}>{a.label}</option>
        ))}
      </select>
      {error && <Text size="small" className="field-error">{error}</Text>}
    </div>
  );
}
