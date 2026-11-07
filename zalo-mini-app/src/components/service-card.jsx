import React from "react";
import { Box, Button, Icon, Text } from "zmp-ui";

import { api } from "../api";
import { T } from "../texts";
import { phone } from "../format";
import { callProvider } from "../zalo";

/**
 * A service near the farmer: photo, name, provider, distance, price note and
 * one big call button (counted like the website's). `service` is a
 * `PublicServiceDto` of a near-me search.
 */
export function ServiceCard({ service }) {
  return (
    <Box className="card service-card">
      <div className="service-card__row">
        {service.image_file_id ? (
          <img className="service-card__photo" src={api.thumbUrl(service.image_file_id)} alt="" loading="lazy" />
        ) : (
          <span className="service-card__photo service-card__photo--empty"><Icon icon="zi-photo" /></span>
        )}
        <div className="service-card__text">
          <Text.Title size="small">{service.name}</Text.Title>
          <Text size="small">
            {service.business_name}
            {service.business_verified && <span className="verified"> · {T.verified}</span>}
          </Text>
          <Text size="xSmall" className="muted">
            {service.category}
            {typeof service.distance_m === "number" && ` · ${T.km(service.distance_m)}`}
          </Text>
          {service.price_note && <Text size="small">{service.price_note}</Text>}
        </div>
      </div>
      <Button
        fullWidth
        prefixIcon={<Icon icon="zi-call" />}
        onClick={() => callProvider(service.business_phone, service.id)}
      >
        {T.call(phone(service.business_phone))}
      </Button>
    </Box>
  );
}
