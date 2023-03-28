// Copyright 2022 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import {
  Box,
  Button,
  Card,
  CardActions,
  CardContent,
  Typography,
} from "@mui/material";
import { graphql, useLazyLoadQuery } from "react-relay";
import BrowserSessionList from "../components/BrowserSessionList";

import CompatSsoLoginList from "../components/CompatSsoLoginList";
import OAuth2SessionList from "../components/OAuth2SessionList";
import type { PersonalInfoQuery } from "./__generated__/PersonalInfoQuery.graphql";

const PersonalInfo: React.FC = () => {
  const data = useLazyLoadQuery<PersonalInfoQuery>(
    graphql`
      query PersonalInfoQuery {
        currentUser {
          id
          username
          primaryEmail {
            id
            email
            confirmedAt
          }
        }
      }
    `,
    {}
  );

  if (data.currentUser) {
    const user = data.currentUser;

    return (
      <>
        <Typography variant="h1">Personal info</Typography>
        <Box>
          <Card>
            <CardContent>
              <Typography gutterBottom variant="h5" component="div">
                Lizard
              </Typography>
              <Typography variant="body2" color="text.secondary">
                Lizards are a widespread group of squamate reptiles, with over
                6,000 species, ranging across all continents except Antarctica
              </Typography>
            </CardContent>
            <CardActions>
              <Button size="small">Share</Button>
              <Button size="small">Learn More</Button>
            </CardActions>
          </Card>
        </Box>
      </>
    );
  } else {
    return <div className="font-bold text-alert">You're not logged in.</div>;
  }
};

export default PersonalInfo;
