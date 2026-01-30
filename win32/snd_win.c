/*
Copyright (C) 1997-2001 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  

See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program; if not, write to the Free Software
Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.

*/
#include <float.h>

#include "../client/client.h"
#include "../client/snd_loc.h"
#include "../qcommon/myq2opts.h" // mattx86: myq2opts.h
#include "winquake.h"

#define iDirectSoundCreate(a,b,c)	pDirectSoundCreate(a,b,c)

HRESULT (WINAPI *pDirectSoundCreate)(GUID FAR *lpGUID, LPDIRECTSOUND FAR *lplpDS, IUnknown FAR *pUnkOuter);

// 64K is > 1 second at 16-bit, 22050 Hz
#define	WAV_BUFFERS				64
#define	WAV_MASK				0x3F
#define	WAV_BUFFER_SIZE			0x0400
#define SECONDARY_BUFFER_SIZE	0x10000

typedef enum {SIS_SUCCESS, SIS_FAILURE, SIS_NOTAVAIL} sndinitstat;

cvar_t	*s_wavonly;

static qboolean	dsound_init;
static qboolean	wav_init;
static qboolean	snd_firsttime = true, snd_isdirect, snd_iswave;
static qboolean	primary_format_set;

// starts at 0 for disabled
static int	snd_buffer_count = 0;
static int	sample16;
static int	snd_sent, snd_completed;

/* 
 * Global variables. Must be visible to window-procedure function 
 *  so it can unlock and free the data block after it has been played. 
 */ 


HANDLE		hData;
HPSTR		lpData, lpData2;

HGLOBAL		hWaveHdr;
LPWAVEHDR	lpWaveHdr;

HWAVEOUT    hWaveOut; 

WAVEOUTCAPS	wavecaps;

DWORD	gSndBufSize;

MMTIME		mmstarttime;

LPDIRECTSOUND pDS;
LPDIRECTSOUNDBUFFER pDSBuf, pDSPBuf;

HINSTANCE hInstDS;

qboolean SNDDMA_InitDirect (void);
qboolean SNDDMA_InitWav (void);

void FreeSound( void );

static const char *DSoundError( int error )
{
	switch ( error )
	{
	case DSERR_BUFFERLOST:
		return "DSERR_BUFFERLOST";
	case DSERR_INVALIDCALL:
		return "DSERR_INVALIDCALLS";
	case DSERR_INVALIDPARAM:
		return "DSERR_INVALIDPARAM";
	case DSERR_PRIOLEVELNEEDED:
		return "DSERR_PRIOLEVELNEEDED";
	}

	return "unknown";
}

/*
** DS_CreateBuffers
*/
static qboolean DS_CreateBuffers( void )
{
	DSBUFFERDESC	dsbuf;
	DSBCAPS			dsbcaps;
	WAVEFORMATEX	pformat, format;
	DWORD			dwWrite;

	memset (&format, 0, sizeof(format));
	format.wFormatTag = WAVE_FORMAT_PCM;
    format.nChannels = dma.channels;
    format.wBitsPerSample = dma.samplebits;
    format.nSamplesPerSec = dma.speed;
    format.nBlockAlign = format.nChannels * format.wBitsPerSample / 8;
    format.cbSize = 0;
    format.nAvgBytesPerSec = format.nSamplesPerSec*format.nBlockAlign; 

	S_Printf(PRINT_INFO, "Creating DS buffers\n" );

	S_Printf(PRINT_DEVELOPER, "...setting EXCLUSIVE coop level: " );
	if ( DS_OK != pDS->lpVtbl->SetCooperativeLevel( pDS, cl_hwnd, DSSCL_EXCLUSIVE ) )
	{
		S_Printf(PRINT_INFO, "failed\n");
		FreeSound ();
		return false;
	}
	S_Printf(PRINT_DEVELOPER, "ok\n" );

// get access to the primary buffer, if possible, so we can set the
// sound hardware format
	memset (&dsbuf, 0, sizeof(dsbuf));
	dsbuf.dwSize = sizeof(DSBUFFERDESC);
	dsbuf.dwFlags = DSBCAPS_PRIMARYBUFFER;
	dsbuf.dwBufferBytes = 0;
	dsbuf.lpwfxFormat = NULL;

	memset(&dsbcaps, 0, sizeof(dsbcaps));
	dsbcaps.dwSize = sizeof(dsbcaps);
	primary_format_set = false;

	S_Printf(PRINT_DEVELOPER, "...creating primary buffer: " );
	if (DS_OK == pDS->lpVtbl->CreateSoundBuffer(pDS, &dsbuf, &pDSPBuf, NULL))
	{
		pformat = format;

		S_Printf(PRINT_DEVELOPER, "ok\n" );
		if (DS_OK != pDSPBuf->lpVtbl->SetFormat (pDSPBuf, &pformat))
		{
			if (snd_firsttime)
				S_Printf(PRINT_DEVELOPER, "...setting primary sound format: failed\n");
		}
		else
		{
			if (snd_firsttime)
				S_Printf(PRINT_DEVELOPER, "...setting primary sound format: ok\n");

			primary_format_set = true;
		}
	}
	else
		S_Printf(PRINT_INFO, "failed\n" );

	if ( !primary_format_set || !s_primary->value)
	{
	// create the secondary buffer we'll actually work with
		memset (&dsbuf, 0, sizeof(dsbuf));
		dsbuf.dwSize = sizeof(DSBUFFERDESC);
		dsbuf.dwFlags = DSBCAPS_CTRLFREQUENCY | DSBCAPS_LOCSOFTWARE;
		dsbuf.dwBufferBytes = SECONDARY_BUFFER_SIZE;
		dsbuf.lpwfxFormat = &format;

		memset(&dsbcaps, 0, sizeof(dsbcaps));
		dsbcaps.dwSize = sizeof(dsbcaps);

		S_Printf(PRINT_DEVELOPER, "...creating secondary buffer: " );
		if (DS_OK != pDS->lpVtbl->CreateSoundBuffer(pDS, &dsbuf, &pDSBuf, NULL))
		{
			S_Printf(PRINT_INFO, "failed\n" );
			FreeSound ();
			return false;
		}
		S_Printf(PRINT_DEVELOPER, "ok\n" );

		dma.channels = format.nChannels;
		dma.samplebits = format.wBitsPerSample;
		dma.speed = format.nSamplesPerSec;

		if (DS_OK != pDSBuf->lpVtbl->GetCaps (pDSBuf, &dsbcaps))
		{
			S_Printf(PRINT_INFO, "*** GetCaps failed ***\n");
			FreeSound ();
			return false;
		}

		S_Printf(PRINT_INFO, "...using secondary sound buffer\n");
	}
	else
	{
		S_Printf(PRINT_INFO, "...using primary buffer\n" );

		S_Printf(PRINT_DEVELOPER, "...setting WRITEPRIMARY coop level: " );
		if (DS_OK != pDS->lpVtbl->SetCooperativeLevel (pDS, cl_hwnd, DSSCL_WRITEPRIMARY))
		{
			S_Printf(PRINT_INFO, "failed\n" );
			FreeSound ();
			return false;
		}
		S_Printf(PRINT_DEVELOPER, "ok\n" );

		if (DS_OK != pDSPBuf->lpVtbl->GetCaps (pDSPBuf, &dsbcaps))
		{
			S_Printf(PRINT_INFO, "*** GetCaps failed ***\n");
			return false;
		}

		pDSBuf = pDSPBuf;
	}

	// Make sure mixer is active
	pDSBuf->lpVtbl->Play(pDSBuf, 0, 0, DSBPLAY_LOOPING);

	if (snd_firsttime)
		S_Printf(PRINT_INFO, "   %d channel(s)\n"
							"   %d bits/sample\n"
							"   %d bytes/sec\n",
							dma.channels, dma.samplebits, dma.speed);
	
	gSndBufSize = dsbcaps.dwBufferBytes;

	/* we don't want anyone to access the buffer directly w/o locking it first. */
	lpData = NULL; 

	pDSBuf->lpVtbl->Stop(pDSBuf);
	pDSBuf->lpVtbl->GetCurrentPosition(pDSBuf, &mmstarttime.u.sample, &dwWrite);
	pDSBuf->lpVtbl->Play(pDSBuf, 0, 0, DSBPLAY_LOOPING);

	dma.samples = gSndBufSize/(dma.samplebits/8);
	dma.samplepos = 0;
	dma.submission_chunk = 1;
	dma.buffer = (unsigned char *) lpData;
	sample16 = (dma.samplebits/8) - 1;

	return true;
}

/*
** DS_DestroyBuffers
*/
static void DS_DestroyBuffers( void )
{
	S_Printf(PRINT_DEVELOPER, "Destroying DS buffers\n" );
	if ( pDS )
	{
		S_Printf(PRINT_DEVELOPER, "...setting NORMAL coop level\n" );
		pDS->lpVtbl->SetCooperativeLevel( pDS, cl_hwnd, DSSCL_NORMAL );
	}

	if ( pDSBuf )
	{
		S_Printf(PRINT_DEVELOPER, "...stopping and releasing sound buffer\n" );
		pDSBuf->lpVtbl->Stop( pDSBuf );
		pDSBuf->lpVtbl->Release( pDSBuf );
	}

	// only release primary buffer if it's not also the mixing buffer we just released
	if ( pDSPBuf && ( pDSBuf != pDSPBuf ) )
	{
		S_Printf(PRINT_DEVELOPER, "...releasing primary buffer\n" );
		pDSPBuf->lpVtbl->Release( pDSPBuf );
	}
	pDSBuf = NULL;
	pDSPBuf = NULL;

	dma.buffer = NULL;
}

/*
==================
FreeSound
==================
*/
void FreeSound (void)
{
	int		i;

	S_Printf(PRINT_DEVELOPER, "Shutting down sound system\n" );

	if ( pDS )
		DS_DestroyBuffers();

	if ( hWaveOut )
	{
		S_Printf(PRINT_DEVELOPER, "...resetting waveOut\n" );
		waveOutReset (hWaveOut);

		if (lpWaveHdr)
		{
			S_Printf(PRINT_DEVELOPER, "...unpreparing headers\n" );
			for (i=0 ; i< WAV_BUFFERS ; i++)
				waveOutUnprepareHeader (hWaveOut, lpWaveHdr+i, sizeof(WAVEHDR));
		}

		S_Printf(PRINT_DEVELOPER, "...closing waveOut\n" );
		waveOutClose (hWaveOut);

		if (hWaveHdr)
		{
			S_Printf(PRINT_DEVELOPER, "...freeing WAV header\n" );
			GlobalUnlock(hWaveHdr);
			GlobalFree(hWaveHdr);
		}

		if (hData)
		{
			S_Printf(PRINT_DEVELOPER, "...freeing WAV buffer\n" );
			GlobalUnlock(hData);
			GlobalFree(hData);
		}

	}

	if ( pDS )
	{
		S_Printf(PRINT_DEVELOPER, "...releasing DS object\n" );
		pDS->lpVtbl->Release( pDS );
	}

	if ( hInstDS )
	{
		S_Printf(PRINT_DEVELOPER, "...freeing DSOUND.DLL\n" );
		FreeLibrary( hInstDS );
		hInstDS = NULL;
	}

	pDS = NULL;
	pDSBuf = NULL;
	pDSPBuf = NULL;
	hWaveOut = 0;
	hData = 0;
	hWaveHdr = 0;
	lpData = NULL;
	lpWaveHdr = NULL;
	dsound_init = false;
	wav_init = false;
}

/*
==================
SNDDMA_InitDirect

Direct-Sound support
==================
*/
sndinitstat SNDDMA_InitDirect (void)
{
	DSCAPS			dscaps;
	HRESULT			hresult;

	dma.channels = 2;
	dma.samplebits = 16;

	if (s_khz->value == 44)
		dma.speed = 44100;
	else if (s_khz->value == 22) // mattx86: 44khz_fix
		dma.speed = 22050;
	else
		dma.speed = 11025;

	S_Printf(PRINT_INFO, "Initializing DirectSound\n");

	if ( !hInstDS )
	{
		S_Printf(PRINT_DEVELOPER, "...loading dsound.dll: " );

		hInstDS = LoadLibrary("dsound.dll");
		
		if (hInstDS == NULL)
		{
			S_Printf(PRINT_INFO, "failed\n");
			return SIS_FAILURE;
		}

		S_Printf(PRINT_DEVELOPER, "ok\n");
		pDirectSoundCreate = (void *)GetProcAddress(hInstDS,"DirectSoundCreate");

		if (!pDirectSoundCreate)
		{
			S_Printf(PRINT_INFO, "*** couldn't get DS proc addr ***\n");
			return SIS_FAILURE;
		}
	}

	S_Printf(PRINT_DEVELOPER, "...creating DS object: " );
	while ( ( hresult = iDirectSoundCreate( NULL, &pDS, NULL ) ) != DS_OK )
	{
		if (hresult != DSERR_ALLOCATED)
		{
			S_Printf(PRINT_INFO, "failed\n" );
			return SIS_FAILURE;
		}

		if (MessageBox (NULL,
						"The sound hardware is in use by another app.\n\n"
					    "Select Retry to try to start sound again or Cancel to run Quake with no sound.",
						"Sound not available",
						MB_RETRYCANCEL | MB_SETFOREGROUND | MB_ICONEXCLAMATION) != IDRETRY)
		{
			S_Printf(PRINT_INFO, "failed, hardware already in use\n" );
			return SIS_NOTAVAIL;
		}
	}
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	dscaps.dwSize = sizeof(dscaps);

	if ( DS_OK != pDS->lpVtbl->GetCaps( pDS, &dscaps ) )
	{
		S_Printf(PRINT_INFO, "*** couldn't get DS caps ***\n");
	}

	if ( dscaps.dwFlags & DSCAPS_EMULDRIVER )
	{
		S_Printf(PRINT_DEVELOPER, "...no DSound driver found\n" );
		FreeSound();
		return SIS_FAILURE;
	}

	if ( !DS_CreateBuffers() )
		return SIS_FAILURE;

	dsound_init = true;

	S_Printf(PRINT_DEVELOPER, "...completed successfully\n" );

	return SIS_SUCCESS;
}


/*
==================
SNDDM_InitWav

Crappy windows multimedia base
==================
*/
qboolean SNDDMA_InitWav (void)
{
	WAVEFORMATEX  format; 
	int				i;
	HRESULT			hr;

	S_Printf(PRINT_INFO, "Initializing wave sound\n" );
	
	snd_sent = 0;
	snd_completed = 0;

	dma.channels = 2;
	dma.samplebits = 16;

	if (s_khz->value == 44)
		dma.speed = 44100;
	else if (s_khz->value == 22) // mattx86: 44khz_fix
		dma.speed = 22050;
	else
		dma.speed = 11025;

	memset (&format, 0, sizeof(format));
	format.wFormatTag = WAVE_FORMAT_PCM;
	format.nChannels = dma.channels;
	format.wBitsPerSample = dma.samplebits;
	format.nSamplesPerSec = dma.speed;
	format.nBlockAlign = format.nChannels
		*format.wBitsPerSample / 8;
	format.cbSize = 0;
	format.nAvgBytesPerSec = format.nSamplesPerSec
		*format.nBlockAlign; 
	
	/* Open a waveform device for output using window callback. */ 
	S_Printf(PRINT_DEVELOPER, "...opening waveform device: ");
	while ((hr = waveOutOpen((LPHWAVEOUT)&hWaveOut, WAVE_MAPPER, 
					&format, 
					0, 0L, CALLBACK_NULL)) != MMSYSERR_NOERROR)
	{
		if (hr != MMSYSERR_ALLOCATED)
		{
			S_Printf(PRINT_INFO, "failed\n");
			return false;
		}

		if (MessageBox (NULL,
						"The sound hardware is in use by another app.\n\n"
					    "Select Retry to try to start sound again or Cancel to run Quake 2 with no sound.",
						"Sound not available",
						MB_RETRYCANCEL | MB_SETFOREGROUND | MB_ICONEXCLAMATION) != IDRETRY)
		{
			S_Printf(PRINT_INFO, "hw in use\n" );
			return false;
		}
	} 
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	/* 
	 * Allocate and lock memory for the waveform data. The memory 
	 * for waveform data must be globally allocated with 
	 * GMEM_MOVEABLE and GMEM_SHARE flags. 

	*/ 
	S_Printf(PRINT_DEVELOPER, "...allocating waveform buffer: ");
	gSndBufSize = WAV_BUFFERS*WAV_BUFFER_SIZE;
	hData = GlobalAlloc(GMEM_MOVEABLE | GMEM_SHARE, gSndBufSize); 
	if (!hData) 
	{
		S_Printf(PRINT_INFO, " failed\n" );
		FreeSound ();
		return false; 
	}
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	S_Printf(PRINT_DEVELOPER, "...locking waveform buffer: ");
	lpData = GlobalLock(hData);
	if (!lpData)
	{ 
		S_Printf(PRINT_INFO, " failed\n" );
		FreeSound ();
		return false; 
	} 
	memset (lpData, 0, gSndBufSize);
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	/* 
	 * Allocate and lock memory for the header. This memory must 
	 * also be globally allocated with GMEM_MOVEABLE and 
	 * GMEM_SHARE flags. 
	 */ 
	S_Printf(PRINT_DEVELOPER, "...allocating waveform header: ");
	hWaveHdr = GlobalAlloc(GMEM_MOVEABLE | GMEM_SHARE, 
		(DWORD) sizeof(WAVEHDR) * WAV_BUFFERS); 

	if (hWaveHdr == NULL)
	{ 
		S_Printf(PRINT_INFO, "failed\n" );
		FreeSound ();
		return false; 
	} 
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	S_Printf(PRINT_DEVELOPER, "...locking waveform header: ");
	lpWaveHdr = (LPWAVEHDR) GlobalLock(hWaveHdr); 

	if (lpWaveHdr == NULL)
	{ 
		S_Printf(PRINT_INFO, "failed\n" );
		FreeSound ();
		return false; 
	}
	memset (lpWaveHdr, 0, sizeof(WAVEHDR) * WAV_BUFFERS);
	S_Printf(PRINT_DEVELOPER, "ok\n" );

	/* After allocation, set up and prepare headers. */ 
	S_Printf(PRINT_DEVELOPER, "...preparing headers: ");
	for (i=0 ; i<WAV_BUFFERS ; i++)
	{
		lpWaveHdr[i].dwBufferLength = WAV_BUFFER_SIZE; 
		lpWaveHdr[i].lpData = lpData + i*WAV_BUFFER_SIZE;

		if (waveOutPrepareHeader(hWaveOut, lpWaveHdr+i, sizeof(WAVEHDR)) !=
				MMSYSERR_NOERROR)
		{
			S_Printf(PRINT_INFO, "failed\n");
			FreeSound ();
			return false;
		}
	}
	S_Printf(PRINT_DEVELOPER, "ok\n");

	dma.samples = gSndBufSize/(dma.samplebits/8);
	dma.samplepos = 0;
	dma.submission_chunk = 512;
	dma.buffer = (unsigned char *) lpData;
	sample16 = (dma.samplebits/8) - 1;

	wav_init = true;

	return true;
}

/*
==================
SNDDMA_Init

Try to find a sound device to mix for.
Returns false if nothing is found.
==================
*/
int SNDDMA_Init(void)
{
	sndinitstat	stat;

	memset ((void *)&dma, 0, sizeof (dma));

	s_wavonly = Cvar_Get ("s_wavonly", "0", CVAR_ARCHIVE);

	dsound_init = wav_init = 0;

	stat = SIS_FAILURE;	// assume DirectSound won't initialize

	/* Init DirectSound */
	if (!s_wavonly->value)
	{
		if (snd_firsttime || snd_isdirect)
		{
			stat = SNDDMA_InitDirect ();

			if (stat == SIS_SUCCESS)
			{
				snd_isdirect = true;

				if (snd_firsttime)
				{
					S_Printf(PRINT_INFO, "dsound init succeeded\n" );
				}
			}
			else
			{
				snd_isdirect = false;
				S_Printf(PRINT_INFO, "*** dsound init failed ***\n");
			}
		}
	}

// if DirectSound didn't succeed in initializing, try to initialize
// waveOut sound, unless DirectSound failed because the hardware is
// already allocated (in which case the user has already chosen not
// to have sound)
	if (!dsound_init && (stat != SIS_NOTAVAIL))
	{
		if (snd_firsttime || snd_iswave)
		{

			snd_iswave = SNDDMA_InitWav ();

			if (snd_iswave)
			{
				if (snd_firsttime)
					S_Printf(PRINT_INFO, "Wave sound init succeeded\n");
			}
			else
			{
				S_Printf(PRINT_INFO, "Wave sound init failed\n");
			}
		}
	}

	snd_firsttime = false;

	snd_buffer_count = 1;

	if (!dsound_init && !wav_init)
	{
		if (snd_firsttime)
			S_Printf(PRINT_INFO, "*** No sound device initialized ***\n");

		return 0;
	}

	return 1;
}

/*
==============
SNDDMA_GetDMAPos

return the current sample position (in mono samples read)
inside the recirculating dma buffer, so the mixing code will know
how many sample are required to fill it up.
===============
*/
int SNDDMA_GetDMAPos(void)
{
	MMTIME	mmtime;
	int		s;
	DWORD	dwWrite;

	if (dsound_init) 
	{
		mmtime.wType = TIME_SAMPLES;
		pDSBuf->lpVtbl->GetCurrentPosition(pDSBuf, &mmtime.u.sample, &dwWrite);
		s = mmtime.u.sample - mmstarttime.u.sample;
	}
	else if (wav_init)
	{
		s = snd_sent * WAV_BUFFER_SIZE;
	}


	s >>= sample16;

	s &= (dma.samples-1);

	return s;
}

/*
==============
SNDDMA_BeginPainting

Makes sure dma.buffer is valid
===============
*/
DWORD	locksize;
void SNDDMA_BeginPainting (void)
{
	int		reps;
	DWORD	dwSize2;
	DWORD	*pbuf, *pbuf2;
	HRESULT	hresult;
	DWORD	dwStatus;

	if (!pDSBuf)
		return;

	// if the buffer was lost or stopped, restore it and/or restart it
	if (pDSBuf->lpVtbl->GetStatus (pDSBuf, &dwStatus) != DS_OK)
		S_Printf(PRINT_INFO, "Couldn't get sound buffer status\n");
	
	if (dwStatus & DSBSTATUS_BUFFERLOST)
		pDSBuf->lpVtbl->Restore (pDSBuf);
	
	if (!(dwStatus & DSBSTATUS_PLAYING))
		pDSBuf->lpVtbl->Play(pDSBuf, 0, 0, DSBPLAY_LOOPING);

	// lock the dsound buffer

	reps = 0;
	dma.buffer = NULL;

	while ((hresult = pDSBuf->lpVtbl->Lock(pDSBuf, 0, gSndBufSize, &pbuf, &locksize, 
								   &pbuf2, &dwSize2, 0)) != DS_OK)
	{
		if (hresult != DSERR_BUFFERLOST)
		{
			S_Printf(PRINT_INFO, "S_TransferStereo16: Lock failed with error '%s'\n", DSoundError( hresult ) );
			S_Shutdown ();
			return;
		}
		else
		{
			pDSBuf->lpVtbl->Restore( pDSBuf );
		}

		if (++reps > 2)
			return;
	}
	dma.buffer = (unsigned char *)pbuf;
}

/*
==============
SNDDMA_Submit

Send sound to device if buffer isn't really the dma buffer
Also unlocks the dsound buffer
===============
*/
void SNDDMA_Submit(void)
{
	LPWAVEHDR	h;
	int			wResult;

	if (!dma.buffer)
		return;

	// unlock the dsound buffer
	if (pDSBuf)
		pDSBuf->lpVtbl->Unlock(pDSBuf, dma.buffer, locksize, NULL, 0);

	if (!wav_init)
		return;

	//
	// find which sound blocks have completed
	//
	while (1)
	{
		if ( snd_completed == snd_sent )
		{
			S_Printf(PRINT_DEVELOPER, "Sound overrun\n");
			break;
		}

		if ( ! (lpWaveHdr[ snd_completed & WAV_MASK].dwFlags & WHDR_DONE) )
		{
			break;
		}

		snd_completed++;	// this buffer has been played
	}

//S_Printf(PRINT_ALL, "completed %i\n", snd_completed);
	//
	// submit a few new sound blocks
	//
	while (((snd_sent - snd_completed) >> sample16) < 8)
	{
		h = lpWaveHdr + ( snd_sent&WAV_MASK );
	if (paintedtime/256 <= snd_sent)
		break;	//	S_Printf(PRINT_ALL, "submit overrun\n");
//S_Printf(PRINT_ALL, "send %i\n", snd_sent);
		snd_sent++;
		/* 
		 * Now the data block can be sent to the output device. The 
		 * waveOutWrite function returns immediately and waveform 
		 * data is sent to the output device in the background. 
		 */ 
		wResult = waveOutWrite(hWaveOut, h, sizeof(WAVEHDR)); 

		if (wResult != MMSYSERR_NOERROR)
		{ 
			S_Printf(PRINT_INFO, "Failed to write block to device\n");
			FreeSound ();
			return; 
		} 
	}
}

/*
==============
SNDDMA_Shutdown

Reset the sound device for exiting
===============
*/
void SNDDMA_Shutdown(void)
{
	FreeSound ();
}


/*
===========
S_Activate

Called when the main window gains or loses focus.
The window have been destroyed and recreated
between a deactivate and an activate.
===========
*/
void S_Activate (qboolean active)
{
	if ( active )
	{
		if ( pDS && cl_hwnd && snd_isdirect )
		{
			DS_CreateBuffers();
		}
	}
	else
	{
		if ( pDS && cl_hwnd && snd_isdirect )
		{
			DS_DestroyBuffers();
		}
	}
}

void S_Printf (int print_level, char *fmt, ...)
{
	va_list		argptr;
	char		msg[4096];
	static qboolean	inupdate;

	va_start (argptr,fmt);
	vsprintf (msg,fmt,argptr);
	va_end (argptr);

	if (print_level == PRINT_ALL)
	{
		Com_Printf ("%s", msg);
	}
	else if (print_level == PRINT_INFO && s_verbose->value)
	{
		Com_Printf("%s", msg);
	}
	else if (print_level == PRINT_DEVELOPER )
	{
		Com_DPrintf ("%s", msg);
	}
}
