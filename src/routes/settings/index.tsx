import { useSearch, useNavigate } from "@tanstack/react-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AiKeyForm } from "@/components/settings/AiKeyForm";
import { AccountsForm } from "@/components/settings/AccountsForm";
import { PublicationForm } from "@/components/settings/PublicationForm";
import { AboutSection } from "@/components/settings/AboutSection";

export function SettingsPage() {
  const { tab } = useSearch({ from: "/settings" });
  const navigate = useNavigate();

  return (
    <div className="p-6">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-foreground">Paramètres</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          Configuration de l'application
        </p>
      </div>

      <Tabs
        value={tab}
        onValueChange={(v) => navigate({ to: "/settings", search: { tab: v } })}
        className="w-full max-w-2xl"
      >
        <TabsList>
          <TabsTrigger value="ai">Intelligence Artificielle</TabsTrigger>
          <TabsTrigger value="accounts">Comptes</TabsTrigger>
          <TabsTrigger value="publication">Publication</TabsTrigger>
          <TabsTrigger value="about">À propos</TabsTrigger>
        </TabsList>

        <TabsContent value="ai" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Clé API (BYOK)</CardTitle>
              <CardDescription>
                Bring Your Own Key — votre clé est stockée dans le trousseau
                système (Windows Credential Manager / macOS Keychain).
                Elle ne quitte jamais votre machine.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <AiKeyForm />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="accounts" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Comptes sociaux</CardTitle>
              <CardDescription>
                Connecte tes comptes pour publier directement depuis Getpostcraft.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <AccountsForm />
            </CardContent>
          </Card>
        </TabsContent>
        <TabsContent value="publication" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">Hébergement des images</CardTitle>
              <CardDescription>
                Service utilisé pour héberger temporairement les images avant publication sur Instagram.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <PublicationForm />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="about" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">À propos</CardTitle>
            </CardHeader>
            <CardContent>
              <AboutSection />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
