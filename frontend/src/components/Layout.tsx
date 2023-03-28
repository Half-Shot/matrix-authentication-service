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
  AppBar,
  Box,
  Button,
  CssBaseline,
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Toolbar,
  Typography,
  useMediaQuery,
  useTheme,
} from "@mui/material";
import { NavLink } from "react-router-dom";
import { AccountCircle, Apps, ContactMail, Lock } from "@mui/icons-material";
import { graphql, useLazyLoadQuery } from "react-relay";

const drawerWidth = 240;

const pages = [
  { name: "Home", path: "/", icon: <AccountCircle /> },
  { name: "Personal info", path: "/personal-info", icon: <ContactMail /> },
  { name: "Security", path: "/security", icon: <Lock /> },
  { name: "Sessions", path: "/sessions", icon: <Apps /> },
];

const Layout: React.FC<{ children?: React.ReactNode }> = ({ children }) => {
  const theme = useTheme();
  const useDrawer = useMediaQuery(theme.breakpoints.up("sm"));

  const data = useLazyLoadQuery<LayoutQuery>(
    graphql`
      query LayoutQuery {
        currentUser {
          id
          username
        }
      }
    `,
    {}
  );

  return (
    <Box sx={{ display: "flex" }}>
      <CssBaseline />
      <AppBar
        position="fixed"
        sx={{ zIndex: (theme) => theme.zIndex.drawer + 1 }}
      >
        <Toolbar>
          <Typography variant="h6" noWrap component="div">
            matrix.org account
          </Typography>
        </Toolbar>
        <Box sx={{ flexGrow: 1, display: { xs: "flex", sm: "none" } }}>
          {pages.map(({ name, path }) => (
            <NavLink to={path} key={path}>
              <Button
                key={path}
                sx={{ my: 2, color: "white", display: "block" }}
              >
                {name}
              </Button>
            </NavLink>
          ))}
        </Box>
      </AppBar>
      <Drawer
        variant={useDrawer ? "permanent" : "temporary"}
        sx={{
          width: drawerWidth,
          flexShrink: 0,
          [`& .MuiDrawer-paper`]: {
            width: drawerWidth,
            boxSizing: "border-box",
          },
        }}
      >
        <Toolbar />
        <Box sx={{ overflow: "auto" }}>
          <List>
            {pages.map(({ name, path, icon }) => (
              <ListItem key={path} disablePadding sx={{ display: "block" }}>
                <ListItemButton component={NavLink} to={path}>
                  {icon && <ListItemIcon>{icon}</ListItemIcon>}
                  <ListItemText primary={name} />
                </ListItemButton>
              </ListItem>
            ))}
          </List>
        </Box>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3 }}>
        <Toolbar />
        {data.currentUser ? (
          children
        ) : (
          <Typography>You're not logged in.</Typography>
        )}
      </Box>
    </Box>
  );
};

export default Layout;
