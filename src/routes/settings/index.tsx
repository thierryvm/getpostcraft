import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { AiKeyForm } from "@/components/settings/AiKeyForm";

export function SettingsPage() {
  return (
    <div className="p-6">
      <div className="mb-6">
        <h1 className="text-xl font-semibold text-foreground">Paramètres</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          Configuration de l'application
        </p>
      </div>

      <Tabs defaultValue="ai" className="w-full max-w-2xl">
        <TabsList>
          <TabsTrigger value="ai">Intelligence Artificielle</TabsTrigger>
          <TabsTrigger value="accounts" disabled>Comptes (V2)</TabsTrigger>
          <TabsTrigger value="about" disabled>À propos</TabsTrigger>
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
      </Tabs>
    </div>
  );
}
