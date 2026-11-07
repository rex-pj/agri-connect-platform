import React from "react";
import { Box, Button, Spinner, Text } from "zmp-ui";

import { T } from "../texts";

/** A load in progress, or its failure with a way to try again. */
export function Loading({ error, onRetry }) {
  if (error) {
    return (
      <Box className="card center">
        <Text>{error}</Text>
        {onRetry && <Button variant="tertiary" onClick={onRetry}>{T.retry}</Button>}
      </Box>
    );
  }
  return (
    <Box className="center">
      <Spinner />
      <Text size="small" className="muted">{T.loading}</Text>
    </Box>
  );
}
