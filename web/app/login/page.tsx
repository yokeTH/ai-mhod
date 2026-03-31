import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import SignIn from "@/components/login/sign-btn";

export default function SignInPage() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-gradient-to-br from-background to-muted p-4">
      <Card className="w-full max-w-md shadow-lg">
        <CardHeader className="space-y-1 text-center">
          <CardTitle className="text-2xl font-bold">Welcome to Chat App</CardTitle>
          <CardDescription>Sign in to continue to your conversations</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col items-center space-y-4">
          <SignIn />
        </CardContent>
        <CardFooter className="flex flex-col space-y-2">
          <div className="text-center text-xs text-muted-foreground">
            By continuing, you agree to our Terms of Service and Privacy Policy.
          </div>
        </CardFooter>
      </Card>
    </div>
  );
}
