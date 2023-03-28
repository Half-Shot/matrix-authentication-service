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
import Grid from "@mui/material/Unstable_Grid2";

import type { HomeQuery } from "./__generated__/HomeQuery.graphql";
import { NavLink } from "react-router-dom";
import {
  AppsTwoTone,
  ContactMailTwoTone,
  LockTwoTone,
} from "@mui/icons-material";

const iconFontSize = "inherit";

const cards = [
  {
    title: "Personal info",
    path: "/personal-info",
    icon: <ContactMailTwoTone fontSize={iconFontSize} />,
    description: "See your profile and contact details",
    link: "Manage your personal info",
  },
  {
    title: "Security",
    path: "/security",
    icon: <LockTwoTone fontSize={iconFontSize} />,
    description: "See your sign in and security settings",
    link: "Manage security",
  },
  {
    title: "Sessions",
    path: "/sessions",
    icon: <AppsTwoTone fontSize={iconFontSize} />,
    description: "See where you're signed in and apps you've authorized",
    link: "Manage devices and apps",
  },
];

const Home: React.FC = () => {
  const data = useLazyLoadQuery<HomeQuery>(
    graphql`
      query HomeQuery {
        currentUser {
          id
          username
        }
      }
    `,
    {}
  );

  if (data.currentUser) {
    const user = data.currentUser;

    return (
      <>
        <Typography variant="h4" align="center">
          Hello {user.username}!
        </Typography>
        <Grid container spacing={2}>
          {cards.map(({ title, description, link, path, icon }) => (
            <Grid key={title} xs={12} sm={6} md={4}>
              <Card>
                <CardContent>
                  <Typography gutterBottom variant="h5" component="div">
                    {title}
                  </Typography>
                  <Box sx={{ display: "flex" }}>
                    <Typography
                      variant="body2"
                      color="text.secondary"
                      sx={{ flexGrow: 1 }}
                    >
                      {description}
                    </Typography>
                    <Box
                      sx={{ fontSize: "4em", color: "secondary.main", ml: 2 }}
                    >
                      {icon}
                    </Box>
                  </Box>
                </CardContent>
                <CardActions>
                  <Button component={NavLink} to={path} size="small">
                    {link}
                  </Button>
                </CardActions>
              </Card>
            </Grid>
          ))}
        </Grid>
      </>
    );
  } else {
    return <div className="font-bold text-alert">You're not logged in.</div>;
  }
};

export default Home;
